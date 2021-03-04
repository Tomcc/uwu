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
    private static Thread thread;
    private static Queue<string> commandqueue = new Queue<string>();

    private static void RunClient() {
        UdpClient receivingUdpClient = new UdpClient(38910);

        //Creates an IPEndPoint to record the IP Address and port number of the sender.
        // The IPEndPoint will allow you to read datagrams sent from any source.
        IPEndPoint RemoteIpEndPoint = new IPEndPoint(IPAddress.Any, 0);

        try {
            while (true) {
                // Blocks until a message returns on this socket from a remote host.
                Byte[] receiveBytes = receivingUdpClient.Receive(ref RemoteIpEndPoint);

                string remoteCommand = Encoding.ASCII.GetString(receiveBytes);

                // push this to be consumed by the main thread
                lock (commandqueue) {
                    commandqueue.Enqueue(remoteCommand);
                }
            }
        } catch (Exception e) {
            Debug.Log(e.ToString());
        }
    }

    private static void OnUpdate() {
        if (EditorApplication.isCompiling) {
            return;
        }

        if (EditorApplication.isUpdating) {
            return;
        }

        lock (commandqueue) {
            while (commandqueue.Count > 0) {
                var cmd = commandqueue.Dequeue();

                if (cmd == "play") {
                    Debug.Log("UWU: Received Play command, entering play mode");
                    EditorApplication.EnterPlaymode();
                }
                else if (cmd == "stop") {
                    Debug.Log("UWU: Received Stop command, stopping play mode");
                    EditorApplication.ExitPlaymode();
                }
                else if (cmd == "refresh") {
                    Debug.Log("UWU: Received asset refresh command"); 
                    AssetDatabase.Refresh();
                    Debug.Log("Done");
                }
                else if (cmd == "build") {
                    Debug.Log("UWU: Received Script build command");
                    UnityEditor.Compilation.CompilationPipeline.RequestScriptCompilation();  
                } else {
                    Debug.LogError("Unknown remote command received '" + cmd + "'");
                }
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
