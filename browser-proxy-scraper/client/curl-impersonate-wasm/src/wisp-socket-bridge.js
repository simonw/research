/**
 * Wisp Socket Bridge — Emscripten JS library that intercepts POSIX socket APIs.
 *
 * This replaces Emscripten's default WebSocket/Fetch networking. BoringSSL
 * inside the WASM module handles all TLS encryption. This bridge only
 * transports raw encrypted bytes over the Wisp WebSocket protocol.
 *
 * Each virtual file descriptor gets its own Wisp stream. Data arrives from
 * the Wisp WebSocket and is queued in a per-fd ring buffer. libcurl's
 * synchronous recv() calls are suspended via Asyncify until data arrives.
 */

mergeInto(LibraryManager.library, {

  // ---- Internal state (not exported to C) ----

  $WispBridge__deps: [],
  $WispBridge: {
    // fd -> { streamId, recvQueue: Uint8Array[], recvResolve: null | Function, closed: bool }
    sockets: {},
    nextFd: 100,
    wispWs: null,       // The Wisp WebSocket connection
    wispUrl: null,      // Wisp server URL
    nextStreamId: 1,
    streamToFd: {},     // streamId -> fd mapping

    // Lazy-init promise: resolved once WebSocket is connected
    initPromise: null,

    // Initialize the Wisp WebSocket connection
    // Auto-reads URL from Module.wispUrl if not provided
    init: function(wispUrl) {
      if (WispBridge.initPromise) return WispBridge.initPromise;
      wispUrl = wispUrl || Module['wispUrl'];
      if (!wispUrl) return Promise.reject(new Error('No wispUrl configured'));
      WispBridge.wispUrl = wispUrl;
      WispBridge.wispWs = new WebSocket(wispUrl);
      WispBridge.wispWs.binaryType = 'arraybuffer';

      WispBridge.wispWs.onmessage = function(event) {
        var data = new Uint8Array(event.data);
        if (data.length < 5) return;

        // Wisp protocol: [type(1)] [streamId(4-LE)] [payload...]
        var packetType = data[0];
        var streamId = data[1] | (data[2] << 8) | (data[3] << 16) | (data[4] << 24);
        var payload = data.slice(5);

        var fd = WispBridge.streamToFd[streamId];
        if (fd === undefined) return;
        var sock = WispBridge.sockets[fd];
        if (!sock) return;

        if (packetType === 0x02) {
          // DATA packet
          if (sock.recvResolve) {
            var resolve = sock.recvResolve;
            sock.recvResolve = null;
            resolve(payload);
          } else {
            sock.recvQueue.push(payload);
          }
        } else if (packetType === 0x04) {
          // CLOSE packet
          sock.closed = true;
          if (sock.recvResolve) {
            var resolve = sock.recvResolve;
            sock.recvResolve = null;
            resolve(null);
          }
        }
      };

      WispBridge.initPromise = new Promise(function(resolve, reject) {
        WispBridge.wispWs.onopen = function() { resolve(); };
        WispBridge.wispWs.onerror = function(e) { reject(e); };
      });
      return WispBridge.initPromise;
    },

    // Ensure connected before any socket operation
    ensureConnected: function() {
      return WispBridge.init();
    },

    // Allocate a virtual fd
    allocFd: function() {
      var fd = WispBridge.nextFd++;
      WispBridge.sockets[fd] = {
        streamId: 0,
        recvQueue: [],
        recvResolve: null,
        closed: false,
        host: '',
        port: 0,
      };
      return fd;
    },

    // Send a Wisp CONNECT packet to open a TCP stream
    sendConnect: function(fd, host, port) {
      var streamId = WispBridge.nextStreamId++;
      var sock = WispBridge.sockets[fd];
      sock.streamId = streamId;
      WispBridge.streamToFd[streamId] = fd;

      // Wisp CONNECT packet: [type=0x01(1)] [streamId(4-LE)] [tcpType=0x01(1)] [port(2-LE)] [hostname...]
      var hostBytes = new TextEncoder().encode(host);
      var packet = new Uint8Array(5 + 1 + 2 + hostBytes.length);
      packet[0] = 0x01; // CONNECT
      packet[1] = streamId & 0xFF;
      packet[2] = (streamId >> 8) & 0xFF;
      packet[3] = (streamId >> 16) & 0xFF;
      packet[4] = (streamId >> 24) & 0xFF;
      packet[5] = 0x01; // TCP
      packet[6] = port & 0xFF;
      packet[7] = (port >> 8) & 0xFF;
      packet.set(hostBytes, 8);

      WispBridge.wispWs.send(packet.buffer);
    },

    // Send data over a Wisp stream
    sendData: function(fd, data) {
      var sock = WispBridge.sockets[fd];
      if (!sock || sock.closed) return -1;

      // Wisp DATA packet: [type=0x02(1)] [streamId(4-LE)] [payload...]
      var packet = new Uint8Array(5 + data.length);
      packet[0] = 0x02; // DATA
      var sid = sock.streamId;
      packet[1] = sid & 0xFF;
      packet[2] = (sid >> 8) & 0xFF;
      packet[3] = (sid >> 16) & 0xFF;
      packet[4] = (sid >> 24) & 0xFF;
      packet.set(data, 5);

      WispBridge.wispWs.send(packet.buffer);
      return data.length;
    },

    // Send a CLOSE packet for a Wisp stream
    sendClose: function(fd) {
      var sock = WispBridge.sockets[fd];
      if (!sock) return;

      // Wisp CLOSE packet: [type=0x04(1)] [streamId(4-LE)] [reason(1)]
      var packet = new Uint8Array(6);
      packet[0] = 0x04; // CLOSE
      var sid = sock.streamId;
      packet[1] = sid & 0xFF;
      packet[2] = (sid >> 8) & 0xFF;
      packet[3] = (sid >> 16) & 0xFF;
      packet[4] = (sid >> 24) & 0xFF;
      packet[5] = 0x02; // reason: voluntary close

      WispBridge.wispWs.send(packet.buffer);

      delete WispBridge.streamToFd[sock.streamId];
      delete WispBridge.sockets[fd];
    },
  },

  // ---- Exported functions (called from C via Asyncify) ----

  // Initialize the bridge from JS before calling curl
  wispBridgeInit: function(urlPtr) {
    var url = UTF8ToString(urlPtr);
    return Asyncify.handleAsync(function() {
      return WispBridge.init(url);
    });
  },
  wispBridgeInit__deps: ['$WispBridge'],
  wispBridgeInit__async: true,

  // socket() replacement
  wispSocketCreate__deps: ['$WispBridge'],
  wispSocketCreate: function() {
    return WispBridge.allocFd();
  },

  // connect() replacement — opens a Wisp TCP stream
  wispSocketConnect__deps: ['$WispBridge'],
  wispSocketConnect__async: true,
  wispSocketConnect: function(fd, hostPtr, port) {
    var host = UTF8ToString(hostPtr);
    return Asyncify.handleAsync(function() {
      return WispBridge.ensureConnected().then(function() {
        WispBridge.sendConnect(fd, host, port);
        return 0;
      });
    });
  },

  // send() replacement — pushes raw bytes into the Wisp stream
  wispSocketSend__deps: ['$WispBridge'],
  wispSocketSend__async: true,
  wispSocketSend: function(fd, bufPtr, len) {
    var data = HEAPU8.slice(bufPtr, bufPtr + len);
    return WispBridge.sendData(fd, data);
  },

  // recv() replacement — pulls bytes from the per-fd receive queue
  // Suspends WASM execution (via Asyncify) until data arrives
  wispSocketRecv__deps: ['$WispBridge'],
  wispSocketRecv__async: true,
  wispSocketRecv: function(fd, bufPtr, maxLen) {
    var sock = WispBridge.sockets[fd];
    if (!sock) return -1;

    return Asyncify.handleAsync(function() {
      return new Promise(function(resolve) {
        // Check if data is already queued
        if (sock.recvQueue.length > 0) {
          var chunk = sock.recvQueue.shift();
          var copyLen = Math.min(chunk.length, maxLen);
          HEAPU8.set(chunk.subarray(0, copyLen), bufPtr);
          // If we didn't consume the full chunk, push remainder back
          if (copyLen < chunk.length) {
            sock.recvQueue.unshift(chunk.subarray(copyLen));
          }
          resolve(copyLen);
          return;
        }

        // Check if connection is closed
        if (sock.closed) {
          resolve(0);
          return;
        }

        // Wait for data
        sock.recvResolve = function(chunk) {
          if (chunk === null) {
            resolve(0); // EOF
            return;
          }
          var copyLen = Math.min(chunk.length, maxLen);
          HEAPU8.set(chunk.subarray(0, copyLen), bufPtr);
          if (copyLen < chunk.length) {
            sock.recvQueue.unshift(chunk.subarray(copyLen));
          }
          resolve(copyLen);
        };
      });
    });
  },

  // close() replacement
  wispSocketClose__deps: ['$WispBridge'],
  wispSocketClose__async: true,
  wispSocketClose: function(fd) {
    WispBridge.sendClose(fd);
    return 0;
  },
});
