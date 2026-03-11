/**
 * Wisp Socket Bridge — Emscripten JS library that intercepts POSIX socket APIs.
 *
 * This replaces Emscripten's default browser networking. TLS still terminates
 * inside curl/BoringSSL in WASM; this bridge only transports raw encrypted
 * bytes over the Wisp WebSocket protocol.
 */

mergeInto(LibraryManager.library, {
  $WispBridge__deps: [],
  $WispBridge: {
    sockets: {},
    nextFd: 100,
    wispWs: null,
    wispUrl: null,
    nextStreamId: 1,
    streamToFd: {},
    initPromise: null,
    defaultSendCredits: null,
    pollResume: null,

    closeReasons: {
      UNKNOWN: 0x01,
      VOLUNTARY: 0x02,
      NETWORK_ERROR: 0x03,
      UNREACHABLE_HOST: 0x42,
      NO_RESPONSE: 0x43,
      CONN_REFUSED: 0x44,
      TRANSFER_TIMEOUT: 0x47,
      HOST_BLOCKED: 0x48,
      CONN_THROTTLED: 0x49,
      CLIENT_ERROR: 0x81,
    },

    debugEnabled: function() {
      return !!(
        (typeof self !== 'undefined' && self.__WISP_BRIDGE_DEBUG) ||
        (typeof Module !== 'undefined' && Module && Module['wispBridgeDebug'])
      );
    },

    log: function(message) {
      if (WispBridge.debugEnabled()) {
        console.log('[wisp-bridge] ' + message);
      }
    },

    asyncifyResumeReturn: function(value) {
      if (typeof Asyncify !== 'undefined' &&
          Asyncify.state === Asyncify.State.Rewinding) {
        return Asyncify.handleAsync(function() {
          return Promise.resolve(value);
        });
      }
      return value;
    },

    hexPreview: function(bytes, limit) {
      var preview = [];
      var max = Math.min(bytes.length, limit || 16);
      for (var i = 0; i < max; i++) {
        var hex = bytes[i].toString(16);
        preview.push(hex.length === 1 ? '0' + hex : hex);
      }
      return preview.join(' ');
    },

    socketSummary: function(sock) {
      return 'state=' + sock.connState +
        ' closed=' + sock.closed +
        ' reason=' + sock.closeReason +
        ' queue=' + sock.recvQueue.length +
        ' credits=' + sock.sendCredits +
        ' pending=' + sock.pendingSends.length;
    },

    allocFd: function() {
      var fd = WispBridge.nextFd++;
      WispBridge.sockets[fd] = {
        streamId: 0,
        recvQueue: [],
        recvResolver: null,
        waiters: [],
        closed: false,
        closeReason: 0,
        host: '',
        port: 0,
        connState: 'IDLE',
        sendCredits: WispBridge.defaultSendCredits,
        pendingSends: [],
        recvResume: null,
        sendResume: null,
        waitResume: null,
      };
      return fd;
    },

    notifyWaiters: function(sock) {
      if (!sock.waiters.length) return;
      var waiters = sock.waiters.slice();
      sock.waiters.length = 0;
      for (var i = 0; i < waiters.length; i++) {
        waiters[i]();
      }
    },

    resolvePendingSends: function(sock, error) {
      if (!sock.pendingSends.length) return;
      var pending = sock.pendingSends.slice();
      sock.pendingSends.length = 0;
      for (var i = 0; i < pending.length; i++) {
        pending[i].resolve(error);
      }
    },

    isErrorReason: function(reason) {
      return reason !== 0 && reason !== WispBridge.closeReasons.VOLUNTARY;
    },

    markSocketClosed: function(sock, reason) {
      sock.closed = true;
      sock.closeReason = reason || 0;
      sock.connState = WispBridge.isErrorReason(sock.closeReason) ? 'ERROR' : 'EOF';
      WispBridge.log('socket closed streamId=' + sock.streamId + ' ' + WispBridge.socketSummary(sock));

      if (sock.recvResolver) {
        var recvResolver = sock.recvResolver;
        sock.recvResolver = null;
        recvResolver(null);
      }

      WispBridge.resolvePendingSends(sock, -1);
      WispBridge.notifyWaiters(sock);
    },

    flushPendingSends: function(sock) {
      if (!sock || sock.closed) {
        return;
      }
      while (sock.pendingSends.length > 0) {
        if (sock.sendCredits === 0) {
          return;
        }

        var next = sock.pendingSends.shift();
        var result = WispBridge.sendPacket(sock, next.data);
        if (result < 0) {
          next.resolve(result);
          return;
        }
        next.resolve(result);
      }
    },

    sendPacket: function(sock, data) {
      if (!sock || sock.closed) return -1;
      if (!WispBridge.wispWs || WispBridge.wispWs.readyState !== 1) return -1;

      var packet = new Uint8Array(5 + data.length);
      packet[0] = 0x02;
      var sid = sock.streamId;
      packet[1] = sid & 0xFF;
      packet[2] = (sid >> 8) & 0xFF;
      packet[3] = (sid >> 16) & 0xFF;
      packet[4] = (sid >> 24) & 0xFF;
      packet.set(data, 5);

      WispBridge.wispWs.send(packet.buffer);
      if (sock.sendCredits !== null && sock.sendCredits > 0) {
        sock.sendCredits--;
      }
      WispBridge.log('sent DATA fd streamId=' + sid + ' len=' + data.length + ' credits=' + sock.sendCredits);
      return data.length;
    },

    computeRevents: function(fd, events) {
      var POLLIN = 0x0001;
      var POLLOUT = 0x0004;
      var POLLERR = 0x0008;
      var POLLHUP = 0x0010;

      if (fd < 0) return 0;
      if (fd < 100) {
        return events & (POLLIN | POLLOUT);
      }

      var sock = WispBridge.sockets[fd];
      if (!sock) return POLLERR;
      if (sock.connState === 'ERROR') return POLLERR;

      var revents = 0;
      if (sock.connState === 'OPEN') {
        if (events & POLLOUT) revents |= POLLOUT;
        if ((events & POLLIN) && sock.recvQueue.length > 0) revents |= POLLIN;
      }

      if (sock.closed) {
        revents |= POLLHUP;
        if (events & POLLIN) revents |= POLLIN;
      }
      return revents;
    },

    hasReadableData: function(sock) {
      return sock && (sock.recvQueue.length > 0 || sock.closed);
    },

    snapshotPoll: function(fdsPtr, nfds) {
      var ready = 0;
      var waitSockets = [];
      var entries = [];
      var details = [];

      for (var i = 0; i < nfds; i++) {
        var base = fdsPtr + i * 8;
        var fd = HEAP32[base >> 2];
        var events = HEAP16[(base + 4) >> 1];
        var revents = WispBridge.computeRevents(fd, events);
        entries.push(revents);
        details.push('fd=' + fd + ' events=0x' + events.toString(16) + ' revents=0x' + revents.toString(16));
        if (revents) {
          ready++;
          continue;
        }
        if (fd >= 100) {
          var sock = WispBridge.sockets[fd];
          if (sock && !sock.closed && sock.connState !== 'ERROR') {
            waitSockets.push(sock);
          }
        }
      }

      return {
        ready: ready,
        waitSockets: waitSockets,
        entries: entries,
        details: details,
      };
    },

    applyPollEntries: function(fdsPtr, nfds, entries) {
      for (var i = 0; i < nfds; i++) {
        var base = fdsPtr + i * 8;
        HEAP16[(base + 6) >> 1] = entries[i] || 0;
      }
    },

    waitForSocketEvent: function(sock, timeoutMs) {
      return new Promise(function(resolve) {
        if (!sock || WispBridge.hasReadableData(sock) || sock.connState === 'ERROR') {
          resolve();
          return;
        }

        var doneCalled = false;
        var timer = null;
        var waiter = function() {
          done();
        };

        function cleanup() {
          if (timer) clearTimeout(timer);
          var idx = sock.waiters.indexOf(waiter);
          if (idx >= 0) {
            sock.waiters.splice(idx, 1);
          }
        }

        function done() {
          if (doneCalled) return;
          doneCalled = true;
          cleanup();
          resolve();
        }

        sock.waiters.push(waiter);

        if (timeoutMs >= 0) {
          timer = setTimeout(done, timeoutMs);
        }
      });
    },

    waitForPollEvent: function(waitSockets, timeoutMs) {
      return new Promise(function(resolve) {
        var doneCalled = false;
        var timer = null;
        var registrations = [];

        function cleanup() {
          if (timer) clearTimeout(timer);
          for (var i = 0; i < registrations.length; i++) {
            var registration = registrations[i];
            var idx = registration.sock.waiters.indexOf(registration.waiter);
            if (idx >= 0) {
              registration.sock.waiters.splice(idx, 1);
            }
          }
          registrations.length = 0;
        }

        function done() {
          if (doneCalled) return;
          doneCalled = true;
          cleanup();
          resolve();
        }

        for (var i = 0; i < waitSockets.length; i++) {
          var sock = waitSockets[i];
          if (!sock) continue;
          if (WispBridge.hasReadableData(sock) || sock.connState === 'ERROR') {
            done();
            return;
          }
          var waiter = done;
          sock.waiters.push(waiter);
          registrations.push({ sock: sock, waiter: waiter });
        }

        if (timeoutMs >= 0) {
          timer = setTimeout(done, timeoutMs);
        }
      });
    },

    doPoll: function(fdsPtr, nfds, timeout) {
      if (WispBridge.pollResume) {
        var resumed = WispBridge.pollResume;
        WispBridge.pollResume = null;
        WispBridge.applyPollEntries(fdsPtr, nfds, resumed.entries);
        WispBridge.log('poll resumed ready=' + resumed.ready + ' [' + resumed.details.join(', ') + ']');
        return WispBridge.asyncifyResumeReturn(resumed.ready);
      }

      var snapshot = WispBridge.snapshotPoll(fdsPtr, nfds);
      WispBridge.applyPollEntries(fdsPtr, nfds, snapshot.entries);
      WispBridge.log('poll nfds=' + nfds + ' timeout=' + timeout + ' ready=' + snapshot.ready + ' waitSockets=' + snapshot.waitSockets.length + ' [' + snapshot.details.join(', ') + ']');

      if (timeout === 0 || snapshot.ready > 0 || snapshot.waitSockets.length === 0) {
        return snapshot.ready;
      }

      return Asyncify.handleAsync(function() {
        return WispBridge.waitForPollEvent(snapshot.waitSockets, timeout).then(function() {
          var resumedSnapshot = WispBridge.snapshotPoll(fdsPtr, nfds);
          WispBridge.pollResume = {
            ready: resumedSnapshot.ready,
            entries: resumedSnapshot.entries,
            details: resumedSnapshot.details,
          };
          WispBridge.log('poll async ready=' + resumedSnapshot.ready + ' timeout=' + timeout + ' [' + resumedSnapshot.details.join(', ') + ']');
          return resumedSnapshot.ready;
        });
      });
    },

    sendConnect: function(fd, host, port) {
      var streamId = WispBridge.nextStreamId++;
      var sock = WispBridge.sockets[fd];
      if (!sock) return -1;

      sock.streamId = streamId;
      sock.host = host;
      sock.port = port;
      sock.connState = 'OPEN';
      sock.sendCredits = WispBridge.defaultSendCredits;
      WispBridge.streamToFd[streamId] = fd;

      var hostBytes = new TextEncoder().encode(host);
      var packet = new Uint8Array(5 + 1 + 2 + hostBytes.length);
      packet[0] = 0x01;
      packet[1] = streamId & 0xFF;
      packet[2] = (streamId >> 8) & 0xFF;
      packet[3] = (streamId >> 16) & 0xFF;
      packet[4] = (streamId >> 24) & 0xFF;
      packet[5] = 0x01;
      packet[6] = port & 0xFF;
      packet[7] = (port >> 8) & 0xFF;
      packet.set(hostBytes, 8);

      WispBridge.wispWs.send(packet.buffer);
      WispBridge.log('CONNECT fd=' + fd + ' streamId=' + streamId + ' host=' + host + ' port=' + port + ' credits=' + sock.sendCredits);
      return 0;
    },

    init: function(wispUrl) {
      if (WispBridge.initPromise) return WispBridge.initPromise;
      wispUrl = wispUrl || Module['wispUrl'];
      if (!wispUrl) return Promise.reject(new Error('No wispUrl configured'));

      WispBridge.wispUrl = wispUrl;
      WispBridge.wispWs = new WebSocket(wispUrl);
      WispBridge.wispWs.binaryType = 'arraybuffer';
      WispBridge.log('init wispUrl=' + wispUrl);

      WispBridge.wispWs.onmessage = function(event) {
        var data = new Uint8Array(event.data);
        if (data.length < 5) return;

        var packetType = data[0];
        var streamId = data[1] | (data[2] << 8) | (data[3] << 16) | (data[4] << 24);
        var payload = data.slice(5);

        if (packetType === 0x03) {
          var credits = payload.length >= 4
            ? (payload[0] | (payload[1] << 8) | (payload[2] << 16) | (payload[3] << 24))
            : 0;
          if (streamId === 0) {
            WispBridge.defaultSendCredits = credits;
            WispBridge.log('received stream-0 CONTINUE credits=' + credits);
            var fds = Object.keys(WispBridge.sockets);
            for (var i = 0; i < fds.length; i++) {
              var sock = WispBridge.sockets[fds[i]];
              if (sock && !sock.closed && sock.sendCredits === null) {
                sock.sendCredits = credits;
                WispBridge.flushPendingSends(sock);
              }
            }
          } else {
            var continueFd = WispBridge.streamToFd[streamId];
            if (continueFd !== undefined) {
              var continueSock = WispBridge.sockets[continueFd];
              if (continueSock) {
                continueSock.sendCredits = credits;
                WispBridge.log('received CONTINUE fd=' + continueFd + ' streamId=' + streamId + ' credits=' + credits);
                WispBridge.flushPendingSends(continueSock);
              }
            }
          }
          return;
        }

        var fd = WispBridge.streamToFd[streamId];
        if (fd === undefined) {
          WispBridge.log('packet for unknown streamId=' + streamId + ' type=0x' + packetType.toString(16));
          return;
        }

        var sock = WispBridge.sockets[fd];
        if (!sock) return;

        if (packetType === 0x02) {
          WispBridge.log('DATA fd=' + fd + ' streamId=' + streamId + ' len=' + payload.length + ' preview=' + WispBridge.hexPreview(payload, 24) + ' ' + WispBridge.socketSummary(sock));
          if (sock.recvResolver) {
            var recvResolver = sock.recvResolver;
            sock.recvResolver = null;
            recvResolver(payload);
          } else {
            sock.recvQueue.push(payload);
          }
          WispBridge.notifyWaiters(sock);
          return;
        }

        if (packetType === 0x04) {
          var reason = payload.length > 0 ? payload[0] : 0;
          WispBridge.log('CLOSE fd=' + fd + ' streamId=' + streamId + ' reason=' + reason);
          WispBridge.markSocketClosed(sock, reason);
        }
      };

      WispBridge.initPromise = new Promise(function(resolve, reject) {
        WispBridge.wispWs.onopen = function() {
          WispBridge.log('WebSocket open');
          resolve();
        };
        WispBridge.wispWs.onerror = function(error) {
          WispBridge.log('WebSocket error');
          reject(error);
        };
        WispBridge.wispWs.onclose = function(event) {
          WispBridge.log('WebSocket close code=' + event.code);
          var fds = Object.keys(WispBridge.sockets);
          for (var i = 0; i < fds.length; i++) {
            var sock = WispBridge.sockets[fds[i]];
            if (sock && !sock.closed) {
              WispBridge.markSocketClosed(sock, WispBridge.closeReasons.NETWORK_ERROR);
            }
          }
        };
      });

      return WispBridge.initPromise;
    },
  },

  $WispBridgeExport__deps: ['$WispBridge'],
  $WispBridgeExport__postset: "Module['wispBridgeInit'] = WispBridge.init;",
  $WispBridgeExport: {},

  wispSocketDebugEnabled__deps: ['$WispBridge'],
  wispSocketDebugEnabled: function() {
    return WispBridge.debugEnabled() ? 1 : 0;
  },

  wispSocketCreate__deps: ['$WispBridge', '$WispBridgeExport'],
  wispSocketCreate: function() {
    var fd = WispBridge.allocFd();
    WispBridge.log('socket created fd=' + fd);
    return fd;
  },

  wispSocketConnect__deps: ['$WispBridge'],
  wispSocketConnect: function(fd, hostPtr, port) {
    var host = UTF8ToString(hostPtr);
    if (WispBridge.wispWs && WispBridge.wispWs.readyState === 1) {
      return WispBridge.sendConnect(fd, host, port);
    }
    WispBridge.log('connect failed fd=' + fd + ' host=' + host + ' port=' + port + ' wsReady=' + (WispBridge.wispWs && WispBridge.wispWs.readyState));
    return -1;
  },

  wispSocketGetState__deps: ['$WispBridge'],
  wispSocketGetState: function(fd) {
    var sock = WispBridge.sockets[fd];
    if (!sock) return 2;
    return sock.connState === 'ERROR' ? 2 : (sock.connState === 'IDLE' ? 0 : 1);
  },

  wispSocketHasData__deps: ['$WispBridge'],
  wispSocketHasData: function(fd) {
    var sock = WispBridge.sockets[fd];
    return WispBridge.hasReadableData(sock) ? 1 : 0;
  },

  wispSocketWaitForData__deps: ['$WispBridge'],
  wispSocketWaitForData__async: true,
  wispSocketWaitForData: function(fd, timeoutMs) {
    var sock = WispBridge.sockets[fd];
    if (!sock) return 0;
    if (sock.waitResume !== null) {
      var resumed = sock.waitResume;
      sock.waitResume = null;
      return WispBridge.asyncifyResumeReturn(resumed);
    }
    if (WispBridge.hasReadableData(sock) || sock.connState === 'ERROR') {
      return 1;
    }
    if (timeoutMs === 0) {
      return 0;
    }
    return Asyncify.handleAsync(function() {
      return WispBridge.waitForSocketEvent(sock, timeoutMs).then(function() {
        var result = (WispBridge.hasReadableData(sock) || sock.connState === 'ERROR') ? 1 : 0;
        sock.waitResume = result;
        return result;
      });
    });
  },

  poll__deps: ['$WispBridge'],
  poll__async: true,
  poll: function(fdsPtr, nfds, timeout) {
    return WispBridge.doPoll(fdsPtr, nfds, timeout);
  },

  __syscall_poll__deps: ['$WispBridge'],
  __syscall_poll__async: true,
  __syscall_poll: function(fdsPtr, nfds, timeout) {
    return WispBridge.doPoll(fdsPtr, nfds, timeout);
  },

  _setitimer_js__sig: 'iid',
  _setitimer_js: function(which, timeoutMs) {
    // Disable emulated POSIX signal timers. In this build they can call back
    // into _emscripten_timeout with a mismatched indirect-call ABI during
    // Asyncify rewind, even when libcurl is configured with CURLOPT_NOSIGNAL.
    WispBridge.log('setitimer disabled which=' + which + ' timeoutMs=' + timeoutMs);
    return 0;
  },

  wispSocketSend__deps: ['$WispBridge'],
  wispSocketSend__async: true,
  wispSocketSend: function(fd, bufPtr, len) {
    var sock = WispBridge.sockets[fd];
    if (!sock) return -1;
    if (sock.sendResume !== null) {
      var resumed = sock.sendResume;
      sock.sendResume = null;
      return WispBridge.asyncifyResumeReturn(resumed);
    }

    var data = HEAPU8.slice(bufPtr, bufPtr + len);
    if (sock.sendCredits === 0) {
      WispBridge.log('queueing send fd=' + fd + ' len=' + len + ' credits=' + sock.sendCredits);
      return Asyncify.handleAsync(function() {
        return new Promise(function(resolve) {
          sock.pendingSends.push({
            data: data,
            resolve: function(result) {
              sock.sendResume = result;
              resolve(result);
            },
          });
          WispBridge.notifyWaiters(sock);
        });
      });
    }

    return WispBridge.sendPacket(sock, data);
  },

  wispSocketRecv__deps: ['$WispBridge'],
  wispSocketRecv__async: true,
  wispSocketRecv: function(fd, bufPtr, maxLen) {
    var sock = WispBridge.sockets[fd];
    if (!sock) return -1;

    function copyChunk(chunk) {
      var copyLen = Math.min(chunk.length, maxLen);
      HEAPU8.set(chunk.subarray(0, copyLen), bufPtr);
      if (copyLen < chunk.length) {
        sock.recvQueue.unshift(chunk.subarray(copyLen));
      }
      return copyLen;
    }

    if (sock.recvResume) {
      var resumed = sock.recvResume;
      sock.recvResume = null;
      if (resumed.kind === 'data') {
        var resumedLen = copyChunk(resumed.chunk);
        WispBridge.log('recv resumed fd=' + fd + ' bytes=' + resumedLen + ' remaining=' + sock.recvQueue.length);
        return WispBridge.asyncifyResumeReturn(resumedLen);
      }
      if (resumed.kind === 'eof') {
        WispBridge.log('recv resumed EOF fd=' + fd);
        return WispBridge.asyncifyResumeReturn(0);
      }
      WispBridge.log('recv resumed error fd=' + fd);
      return WispBridge.asyncifyResumeReturn(-1);
    }

    if (sock.recvQueue.length > 0) {
      var immediate = copyChunk(sock.recvQueue.shift());
      WispBridge.log('recv immediate fd=' + fd + ' bytes=' + immediate + ' remaining=' + sock.recvQueue.length);
      return immediate;
    }

    if (sock.closed) {
      WispBridge.log('recv EOF fd=' + fd);
      return 0;
    }

    if (sock.connState === 'ERROR') {
      WispBridge.log('recv error fd=' + fd);
      return -1;
    }

    return Asyncify.handleAsync(function() {
      return new Promise(function(resolve) {
        sock.recvResolver = function(chunk) {
          sock.recvResolver = null;
          if (chunk === null) {
            WispBridge.log('recv async EOF fd=' + fd);
            var eofResult = sock.connState === 'ERROR' ? -1 : 0;
            sock.recvResume = {
              kind: sock.connState === 'ERROR' ? 'error' : 'eof',
            };
            resolve(eofResult);
            return;
          }
          var asyncLen = Math.min(chunk.length, maxLen);
          sock.recvResume = {
            kind: 'data',
            chunk: chunk,
          };
          WispBridge.log('recv async ready fd=' + fd + ' bytes=' + chunk.length + ' queue=' + sock.recvQueue.length);
          resolve(asyncLen);
        };
      });
    });
  },

  wispSocketClose__deps: ['$WispBridge'],
  wispSocketClose__async: true,
  wispSocketClose: function(fd) {
    var sock = WispBridge.sockets[fd];
    if (!sock) return 0;

    if (WispBridge.wispWs && WispBridge.wispWs.readyState === 1 && sock.streamId) {
      var packet = new Uint8Array(6);
      packet[0] = 0x04;
      packet[1] = sock.streamId & 0xFF;
      packet[2] = (sock.streamId >> 8) & 0xFF;
      packet[3] = (sock.streamId >> 16) & 0xFF;
      packet[4] = (sock.streamId >> 24) & 0xFF;
      packet[5] = WispBridge.closeReasons.VOLUNTARY;
      WispBridge.wispWs.send(packet.buffer);
    }

    delete WispBridge.streamToFd[sock.streamId];
    delete WispBridge.sockets[fd];
    return 0;
  },
});
