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
    class Command {
        string cmd;
        TcpClient client;
        private bool done = false;
        private bool pollAssetRefresh = false;

        public Command(TcpClient client) {
            this.client = client;

            Byte[] bytes = new Byte[256];

            NetworkStream stream = client.GetStream();
            int read = stream.Read(bytes, 0, bytes.Length);
            cmd = System.Text.Encoding.ASCII.GetString(bytes, 0, read);
        }

        void Close(bool success) {
            string responseMsg = success ? "OK" : "ERR";
            byte[] msg = System.Text.Encoding.ASCII.GetBytes(responseMsg);

            client.GetStream().Write(msg, 0, msg.Length);

            client.Close();

            done = true;
        }

        public bool isDone() {
            if (done) {
                return true;
            }

            if (pollAssetRefresh) {
                var working = EditorApplication.isCompiling || EditorApplication.isUpdating;

                if (!working) {
                    Close(true);
                    done = true;
                }
            }

            return done;
        }

        public void Execute() {
            if (cmd == "play") {
                Debug.Log("UWU: Received Play command, entering play mode");
                EditorApplication.EnterPlaymode();
                if (!EditorApplication.isPlaying) {
                    EditorApplication.playModeStateChanged += (PlayModeStateChange state) => {
                        if (state == PlayModeStateChange.EnteredPlayMode) {
                            Close(true);
                        }
                    };
                } else {
                    Close(true);
                }
            } else if (cmd == "stop") {
                Debug.Log("UWU: Received Stop command, stopping play mode");
                if (EditorApplication.isPlaying) {
                    EditorApplication.ExitPlaymode();
                    EditorApplication.playModeStateChanged += (PlayModeStateChange state) => {
                        if (state == PlayModeStateChange.EnteredEditMode) {
                            Close(true);
                        }
                    };
                } else {
                    Close(true);
                }
            } else if (cmd == "refresh") {
                Debug.Log("UWU: Received asset refresh command");
                AssetDatabase.Refresh();
                pollAssetRefresh = true;
            } else if (cmd == "build") {
                Debug.Log("UWU: Received Script build command");
                UnityEditor.Compilation.CompilationPipeline.RequestScriptCompilation();
                UnityEditor.Compilation.CompilationPipeline.compilationFinished += (object o) => {
                    Close(true);
                };
            } else {
                Debug.LogError("Unknown remote command received '" + cmd + "'");
                Close(false);
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
            Debug.Log("UWU: Started listening");
            while (true) {

                TcpClient client = server.AcceptTcpClient();

                // read the command off the socket and move to the main thread
                var command = new Command(client);
                lock (commandqueue) {
                    commandqueue.Enqueue(command);
                }
            }
        } catch (Exception e) {
            Debug.Log(e.ToString());
        }
    }

    private static void OnUpdate() {
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
