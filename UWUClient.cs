using System.Collections.Generic;
using UnityEngine;
using UnityEditor;
using System.Net.Sockets;
using System.Net;
using System;

public enum ExecResult {
    Success,
    Error,
    Wait,
}

static class Util {

    public static bool IsUnityEditorFocused() {
        return UnityEditorInternal.InternalEditorUtility.isApplicationActive;
    }

    public static bool IsUnityEditorBusy() {
        return EditorApplication.isCompiling || EditorApplication.isUpdating;
    }

    public static string ResultToJSON(ExecResult result) {
        switch (result) {
            case ExecResult.Success:
                return "\"Success\"";
            case ExecResult.Error:
                return "\"Error\"";
            case ExecResult.Wait:
                return "\"Wait\"";
            default:
                throw new Exception("Unknown result type");
        }
    }
}

class Command {
    public delegate void MessageSender(ExecResult mode);


    Request request;
    IPAddress requester;
    MessageSender onDone;

    public Command(Request request, IPAddress requester) {
        this.request = request;
        this.requester = requester;
    }

    public string GetCmd() {
        return request.cmd;
    }

    public string GetRequester() {
        return requester.ToString();
    }

    private void WaitEditorNotBusy() {
        if (!Util.IsUnityEditorBusy()) {
            Debug.Log("UWU: Editor is not busy anymore");

            EditorApplication.update -= WaitEditorNotBusy;
            onDone(ExecResult.Success);

            onDone = null;
        }
    }

    private void OkAfterEditorNotBusy(MessageSender onDone) {
        Debug.Log("UWU: Waiting for editor to finish compiling");

        this.onDone = onDone;
        EditorApplication.update += WaitEditorNotBusy;
    }

    public void Execute(MessageSender sender) {
        if (request.cmd == "Play") {
            Debug.Log("UWU: Received Play command, entering play mode");


            if (EditorApplication.isPlaying) {
                Debug.Log("UWU: Already in play mode");
            } else {
                EditorApplication.EnterPlaymode();
            }

            // Play is a bit of a mess. The Editor will kill UWUClient when it enters play mode
            // So we need to return success and then rely on the server to send a *different*
            // command to block until play mode has started
            sender(ExecResult.Success);

        } else if (request.cmd == "CheckAlive") {
            // This is a special command that is used to check if the client has (re)booted
            // for example, when the CLI has requested play mode.
            // It relies on the CLI blocking and retrying until this returns success

            sender(ExecResult.Success);
        } else if (request.cmd == "Stop") {
            Debug.Log("UWU: Received Stop command, stopping play mode");

            if (EditorApplication.isPlaying) {
                sender(ExecResult.Wait);

                EditorApplication.ExitPlaymode();
                EditorApplication.playModeStateChanged += (PlayModeStateChange state) => {
                    if (state == PlayModeStateChange.EnteredEditMode) {
                        sender(ExecResult.Success);
                    }
                };
            } else {
                sender(ExecResult.Success);
            }
        } else if (request.cmd == "Refresh") {
            Debug.Log("UWU: Received asset refresh command");

            // Refresh also sends success immediately and relies on the CLI to block. See Play
            sender(ExecResult.Success);

            AssetDatabase.Refresh();

        } else if (request.cmd == "BackgroundRefresh") {
            Debug.Log("UWU: Received a background refresh command");

            // Refresh also sends success immediately and relies on the CLI to block. See Play
            sender(ExecResult.Success);
            if (!Util.IsUnityEditorFocused()) {

                AssetDatabase.Refresh();
            }
        } else if (request.cmd == "Build") {
            Debug.Log("UWU: Received Script build command");

            sender(ExecResult.Wait);

            UnityEditor.Compilation.CompilationPipeline.RequestScriptCompilation();
            UnityEditor.Compilation.CompilationPipeline.compilationFinished += (object o) => {
                sender(ExecResult.Success);
            };
        } else {
            Debug.LogError("Unknown remote command received '" + request.cmd + "'");
            sender(ExecResult.Error);
        }
    }
}


class Request {
    public string cmd;
    public string id;
}

public static class UWUClient {

    private static Command currentCmd = null;

    // UDP socket
    private static UdpClient udpClient;
    private static IPEndPoint groupEP;

    // a Hash Set of every ID that has already been seem
    private static HashSet<string> seenIds = new HashSet<string>();

    // Fifo queue of Commands to be executed
    private static Queue<Command> commandQueue = new Queue<Command>();

    private static void OnUpdate() {
        // drain the UDP socket so the buffer doesn't back up
        while (udpClient.Available > 0) {
            Byte[] bytes = udpClient.Receive(ref groupEP);
            string cmd = System.Text.Encoding.UTF8.GetString(bytes, 0, bytes.Length);

            // deserialize the request from JSON
            Request request = JsonUtility.FromJson<Request>(cmd);

            Debug.Log("UWU: Received command '" + request.cmd + "'");

            // if the ID has already been seen, ignore it
            if (seenIds.Contains(request.id)) {
                Debug.Log("UWU: Already seen ID " + request.id + ", ignoring");
                continue;
            }

            seenIds.Add(request.id);

            // create a new command and add it to the queue
            Command command = new Command(request, groupEP.Address);

            commandQueue.Enqueue(command);
        }

        if (currentCmd != null || Util.IsUnityEditorBusy()) {
            return;
        }

        // start executing the next command in the queue
        if (commandQueue.Count > 0) {
            currentCmd = commandQueue.Dequeue();

            currentCmd.Execute((ExecResult mode) => {
                // Convert the mode to JSON
                string json = Util.ResultToJSON(mode);

                Debug.Log("UWU: Sending result '" + json + "'");

                // send the result back to the requester
                Byte[] bytes = System.Text.Encoding.UTF8.GetBytes(json);
                udpClient.Send(bytes, bytes.Length, currentCmd.GetRequester().ToString(), groupEP.Port);

                // if mode is not wait, then we are done
                if (mode != ExecResult.Wait) {
                    currentCmd = null;
                }
            });
        }

    }

    [InitializeOnLoadMethod]
    private static void Init() {
        // set up the socket and start listening
        var port = 38910;
        udpClient = new UdpClient(port);
        groupEP = new IPEndPoint(IPAddress.Any, port);

        EditorApplication.update += OnUpdate;

        Debug.Log("UWU: Listening on port " + port);
    }
}
