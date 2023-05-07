using System.Collections.Generic;
using UnityEngine;
using UnityEditor;
using System.Net.Sockets;
using System.Net;
using System;
using System.Text;
using System.Threading;

public enum ExecResult {
    Success,
    Fail,
}

static class Util {

    public static bool IsUnityEditorFocused() {
        return UnityEditorInternal.InternalEditorUtility.isApplicationActive;
    }

    public static bool IsUnityEditorBusy() {
        return EditorApplication.isCompiling || EditorApplication.isUpdating;
    }

    public static byte ModeByte(ExecResult mode) {
        switch (mode) {
            case ExecResult.Success:
                return 0x01;
            case ExecResult.Fail:
                return 0x00;
            default:
                return 0x00;
        }
    }

}

class Command {
    public delegate void OnDoneDelegate(ExecResult mode);


    string cmd;
    OnDoneDelegate onDone;

    public Command(string cmd) {
        this.cmd = cmd;
    }

    public string GetCmd() {
        return cmd;
    }

    private void WaitEditorNotBusy() {
        if (!Util.IsUnityEditorBusy()) {
            Debug.Log("UWU: Editor is not busy anymore");

            EditorApplication.update -= WaitEditorNotBusy;
            onDone(ExecResult.Success);

            onDone = null;
        }
    }

    private void OkAfterEditorNotBusy(OnDoneDelegate onDone) {
        Debug.Log("UWU: Waiting for editor to finish compiling");

        this.onDone = onDone;
        EditorApplication.update += WaitEditorNotBusy;
    }

    public void Execute(OnDoneDelegate onDone) {
        if (cmd == "confirm_restart") {
            Debug.Log("UWU: Confirming alive");

            onDone(ExecResult.Success);
        } else if (cmd == "play") {
            Debug.Log("UWU: Received Play command, entering play mode");

            if (EditorApplication.isPlaying) {
                Debug.Log("UWU: Already in play mode");
                onDone(ExecResult.Success);
            } else {

                EditorApplication.EnterPlaymode();

                OkAfterEditorNotBusy(onDone);
            }
        } else if (cmd == "stop") {
            Debug.Log("UWU: Received Stop command, stopping play mode");

            if (EditorApplication.isPlaying) {
                EditorApplication.ExitPlaymode();
                EditorApplication.playModeStateChanged += (PlayModeStateChange state) => {
                    if (state == PlayModeStateChange.EnteredEditMode) {
                        onDone(ExecResult.Success);
                    }
                };
            } else {
                onDone(ExecResult.Success);
            }
        } else if (cmd == "refresh") {
            Debug.Log("UWU: Received asset refresh command");

            AssetDatabase.Refresh();

            OkAfterEditorNotBusy(onDone);

        } else if (cmd == "background_refresh") {
            Debug.Log("UWU: Received a background refresh command");

            if (Util.IsUnityEditorFocused()) {
                onDone(ExecResult.Success);
            } else {
                AssetDatabase.Refresh();

                OkAfterEditorNotBusy(onDone);
            }
        } else if (cmd == "build") {
            Debug.Log("UWU: Received Script build command");

            UnityEditor.Compilation.CompilationPipeline.RequestScriptCompilation();
            UnityEditor.Compilation.CompilationPipeline.compilationFinished += (object o) => {
                onDone(ExecResult.Success);
            };
        } else {
            Debug.LogError("Unknown remote command received '" + cmd + "'");
            onDone(ExecResult.Fail);
        }
    }
}


public static class UWUClient {
    private static Thread thread;
    private static Queue<Command> recvqueue = new Queue<Command>();
    private static Queue<ExecResult> sendqueue = new Queue<ExecResult>();

    private static Command currentCmd = null;


    private static void RunClient() {
        try {
            Int32 port = 38910;
            IPAddress localAddr = IPAddress.Parse("0.0.0.0");

            // Start listening for client requests.
            var server = new TcpListener(localAddr, port);
            server.Start();

            // Enter the listening loop.
            Debug.Log("UWU: Started listening");
            while (true) {

                TcpClient client = server.AcceptTcpClient();

                Debug.Log("UWU: Accepted connection");

                // begin reading messages from this socket. Multiple connections aren't allowed
                Byte[] bytes = new Byte[256];

                NetworkStream stream = client.GetStream();
                int read = stream.Read(bytes, 0, bytes.Length);
                string cmd = System.Text.Encoding.UTF8.GetString(bytes, 0, read);

                Debug.Log("UWU: Received command '" + cmd + "'");

                lock (recvqueue) {
                    recvqueue.Enqueue(new Command(cmd));
                }

                // wait for the response to send
                while (true) {
                    Thread.Sleep(1000);

                    lock (sendqueue) {
                        if (sendqueue.Count > 0) {

                            ExecResult response = sendqueue.Dequeue();

                            Debug.Log("UWU: Sending response '" + response + "'");

                            byte[] msg = { Util.ModeByte(response) };
                            stream.Write(msg, 0, 1);
                            stream.Flush();

                            break;
                        }
                    }
                }
            }

        } catch (ThreadAbortException) {
            // do nothing. Unity likes to terminate the thread for a number of reasons 
            // and that's fine
        } catch (Exception e) {
            Debug.LogError(e.ToString());
        }
    }

    private static void OnUpdate() {
        if (currentCmd != null || Util.IsUnityEditorBusy()) {
            return;
        }

        var onDone = new Command.OnDoneDelegate((ExecResult result) => {
            Debug.Log("UWU: Command '" + currentCmd.GetCmd() + "' finished with result " + result);

            // Send the response to the server
            sendqueue.Enqueue(result);

            currentCmd = null;
        });

        lock (recvqueue) {
            if (recvqueue.Count > 0) {
                currentCmd = recvqueue.Dequeue();
                currentCmd.Execute(onDone);
            }
        }
    }

    [InitializeOnLoadMethod]
    private static void Init() {
        // kill existing threads if any
        if (thread != null) {
            thread.Abort();
        }

        thread = new Thread(RunClient);
        thread.Start();

        EditorApplication.update += OnUpdate;
    }
}
