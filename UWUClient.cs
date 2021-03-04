using System.Collections;
using System.Collections.Generic;
using UnityEngine;
using UnityEditor;
using System.Net.Sockets;
using System.Net;
using System;
using System.Text;
using System.Threading;

public static class UWUClient {
    static void Log(string msg) {
        // uncomment this for logging
        // Debug.Log(msg);
    }

    class Command {
        enum CloseMode {
            Success,
            Fail,
            ReconnectLater,
        }

        static string ModeString(CloseMode mode) {
            switch (mode) {
                case CloseMode.Success:
                    return "OK";
                case CloseMode.Fail:
                    return "ERR";
                case CloseMode.ReconnectLater:
                    return "RECONNECT";
            }
            return "ERR";
        }

        string cmd;
        TcpClient client;
        private bool done = false;

        public Command(TcpClient client) {
            this.client = client;

            Byte[] bytes = new Byte[256];

            NetworkStream stream = client.GetStream();
            int read = stream.Read(bytes, 0, bytes.Length);
            cmd = System.Text.Encoding.ASCII.GetString(bytes, 0, read);
        }

        void Close(CloseMode mode) {
            var responseMsg = ModeString(mode);
            byte[] msg = System.Text.Encoding.ASCII.GetBytes(responseMsg);

            client.GetStream().Write(msg, 0, msg.Length);

            client.Close();

            done = true;
        }

        public bool isDone() {
            return done;
        }

        public void Execute() {
            if (cmd == "confirm_restart") {
                Log("UWU: Confirming alive");

                Close(CloseMode.Success);
            } else if (cmd == "play") {
                Log("UWU: Received Play command, entering play mode");

                if (!EditorApplication.isPlaying) {
                    Close(CloseMode.ReconnectLater);

                    EditorApplication.EnterPlaymode();
                }
            } else if (cmd == "stop") {
                Log("UWU: Received Stop command, stopping play mode");

                if (EditorApplication.isPlaying) {
                    EditorApplication.ExitPlaymode();
                    EditorApplication.playModeStateChanged += (PlayModeStateChange state) => {
                        if (state == PlayModeStateChange.EnteredEditMode) {
                            Close(CloseMode.Success);
                        }
                    };
                } else {
                    Close(CloseMode.Success);
                }
            } else if (cmd == "refresh") {
                Log("UWU: Received asset refresh command");

                Close(CloseMode.ReconnectLater);
                AssetDatabase.Refresh();
            } else if (cmd == "build") {
                Log("UWU: Received Script build command");

                UnityEditor.Compilation.CompilationPipeline.RequestScriptCompilation();
                UnityEditor.Compilation.CompilationPipeline.compilationFinished += (object o) => {
                    Close(CloseMode.Success);
                };
            } else {
                Debug.LogError("Unknown remote command received '" + cmd + "'");
                Close(CloseMode.Fail);
            }
        }
    }

    private static Thread thread;
    private static Queue<Command> commandqueue = new Queue<Command>();
    private static Command currentCmd = null;

    private static void RunClient() {
        try {
            Int32 port = 38910;
            IPAddress localAddr = IPAddress.Parse("127.0.0.1");

            // Start listening for client requests.
            var server = new TcpListener(localAddr, port);
            server.Start();

            // Enter the listening loop.
            Log("UWU: Started listening");
            while (true) {

                TcpClient client = server.AcceptTcpClient();

                // read the command off the socket and move to the main thread
                var command = new Command(client);
                lock (commandqueue) {
                    commandqueue.Enqueue(command);
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
        if (EditorApplication.isCompiling || EditorApplication.isUpdating) {
            return;
        }

        if (currentCmd != null && !currentCmd.isDone()) {
            return;
        }

        lock (commandqueue) {
            if (commandqueue.Count > 0) {
                currentCmd = commandqueue.Dequeue();
                currentCmd.Execute();
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
