/**
 * LinuxVM - A simple JavaScript library for running Linux commands in a browser VM
 * Built on top of v86 (x86 emulator in WebAssembly)
 */

class LinuxVM {
    constructor() {
        this.emulator = null;
        this.bootComplete = false;
        this.serialBuffer = '';
        this.commandCallbacks = [];
    }

    /**
     * Initialize and start the Linux VM
     * @returns {Promise<void>}
     */
    async startup() {
        return new Promise((resolve, reject) => {
            try {
                // Initialize v86 emulator
                this.emulator = new V86Starter({
                    wasm_path: "assets/v86.wasm",
                    memory_size: 128 * 1024 * 1024, // 128MB
                    vga_memory_size: 8 * 1024 * 1024, // 8MB

                    // Boot from Linux kernel and filesystem
                    bzimage: {
                        url: "assets/bzImage",
                    },
                    initrd: {
                        url: "assets/buildroot.bin",
                    },

                    // Autostart
                    autostart: true,

                    // Disable screen (headless mode)
                    disable_keyboard: true,
                    disable_mouse: true,
                });

                // Listen to serial output
                this.emulator.add_listener("serial0-output-byte", (byte) => {
                    const char = String.fromCharCode(byte);
                    this.serialBuffer += char;

                    // Check for boot completion (login prompt)
                    if (!this.bootComplete && this.serialBuffer.includes("buildroot login:")) {
                        this.bootComplete = true;

                        // Auto-login as root (buildroot default)
                        setTimeout(() => {
                            this.sendToSerial("root\n");

                            // Wait for shell prompt, then resolve
                            setTimeout(() => {
                                resolve();
                            }, 1000);
                        }, 500);
                    }

                    // Check if we have pending command callbacks
                    if (this.commandCallbacks.length > 0) {
                        const callback = this.commandCallbacks[0];
                        callback.output += char;

                        // Check for command completion marker
                        if (callback.output.includes(callback.endMarker)) {
                            this.commandCallbacks.shift();
                            callback.resolve();
                        }
                    }
                });

            } catch (error) {
                reject(error);
            }
        });
    }

    /**
     * Send string to VM serial port
     * @param {string} data
     */
    sendToSerial(data) {
        for (let i = 0; i < data.length; i++) {
            this.emulator.serial0_send(data.charCodeAt(i));
        }
    }

    /**
     * Execute a command in the VM and capture output
     * @param {string} command - Shell command to execute
     * @returns {Promise<{stdout: string, stderr: string}>}
     */
    async execute(command) {
        if (!this.bootComplete) {
            throw new Error("VM not ready. Call startup() first.");
        }

        return new Promise((resolve) => {
            // Generate unique marker for this command
            const marker = `__CMD_END_${Date.now()}__`;
            const endMarker = `EXITCODE:`;

            // Create callback object to track this command
            const callback = {
                output: '',
                endMarker: endMarker,
                resolve: () => {
                    // Parse output
                    const output = callback.output;

                    // Extract stdout (everything before EXITCODE marker)
                    const parts = output.split('EXITCODE:');
                    let stdout = parts[0] || '';

                    // Remove the command echo from output
                    const lines = stdout.split('\n');
                    if (lines.length > 0) {
                        // Remove first line (command echo) and last line (empty)
                        stdout = lines.slice(1, -1).join('\n');
                    }

                    // Clean up any ANSI escape codes and control characters
                    stdout = stdout.replace(/\x1b\[[0-9;]*m/g, ''); // ANSI colors
                    stdout = stdout.replace(/\r/g, ''); // Carriage returns

                    resolve({
                        stdout: stdout,
                        stderr: '' // Serial console doesn't separate stderr easily
                    });
                }
            };

            this.commandCallbacks.push(callback);

            // Send command with completion marker
            // Use a subshell to ensure the marker is printed after command completion
            const wrappedCommand = `(${command}); echo "EXITCODE:$?"\n`;
            this.sendToSerial(wrappedCommand);
        });
    }

    /**
     * Shutdown the VM
     */
    shutdown() {
        if (this.emulator) {
            this.emulator.stop();
        }
    }
}

// Export for use in modules or browser
if (typeof module !== 'undefined' && module.exports) {
    module.exports = LinuxVM;
}
