# DeepSeek Harness Desktop Shell Context

The subset of DeepSeek Harness that gets the interactive harness into a native desktop window on Windows: the desktop shell application, the backend process it launches, and the channel between them.

## Language

**Desktop shell (桌面壳)**:
The Tauri v2 application a user starts by double-clicking an executable; it spawns the backend as a hidden child process and renders the harness UI in an embedded WebView2 window. It is a carrier, not a product surface of its own.
_Avoid_: Client, desktop app, launcher

**Backend**:
The harness process serving the web surface — the `dsh-web-app` bundle composition, started by the shell as a hidden child process with `--profile web --port 0`. It owns sessions, tools, and the model; the shell owns no harness behavior.
_Avoid_: service, server, daemon

**Carrier (载波)**:
A role that communicates with the Host over the wire instead of embedding harness logic — the browser, the desktop shell, ACP, or the JSON-RPC agent.
_Avoid_: Client (reserved for the browser-side plugin layer), client app

**Handshake (握手)**:
The `dsh web:` URL line the backend prints on stdout after its Loader tree settles; the shell parses it to learn the loopback URL when the OS picked the port. `--port 0` defers the port choice to the OS.
_Avoid_: port negotiation, service discovery
