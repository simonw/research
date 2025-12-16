'use client';

import { Session } from '@/lib/types';

interface SessionStatusProps {
    session: Session | null;
    url: string | null;
}

export function SessionStatus({ session, url }: SessionStatusProps) {
    if (!session) {
        return (
            <div className="bg-gray-800/50 rounded-2xl p-8 border border-gray-700">
                <div className="text-center text-gray-400">
                    <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-700/50 flex items-center justify-center">
                        <svg className="w-8 h-8 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 18h.01M8 21h8a2 2 0 002-2V5a2 2 0 00-2-2H8a2 2 0 00-2 2v14a2 2 0 002 2z" />
                        </svg>
                    </div>
                    <p className="text-lg">No active session</p>
                    <p className="text-sm text-gray-500 mt-1">Click "Start Session" to launch an Android emulator</p>
                </div>
            </div>
        );
    }

    const getStatusColor = () => {
        switch (session.status) {
            case 'starting':
                return 'text-yellow-400';
            case 'running':
                return 'text-green-400';
            case 'ended':
                return 'text-gray-400';
            case 'error':
                return 'text-red-400';
            default:
                return 'text-gray-400';
        }
    };

    const getStatusIcon = () => {
        switch (session.status) {
            case 'starting':
                return (
                    <div className="animate-spin rounded-full h-4 w-4 border-2 border-yellow-400 border-t-transparent" />
                );
            case 'running':
                return (
                    <div className="h-4 w-4 rounded-full bg-green-400 animate-pulse" />
                );
            case 'ended':
                return (
                    <div className="h-4 w-4 rounded-full bg-gray-400" />
                );
            case 'error':
                return (
                    <div className="h-4 w-4 rounded-full bg-red-400" />
                );
            default:
                return null;
        }
    };

    const timeRemaining = Math.max(0, session.expiresAt - Date.now());
    const minutes = Math.floor(timeRemaining / 60000);
    const seconds = Math.floor((timeRemaining % 60000) / 1000);

    return (
        <div className="bg-gray-800/50 rounded-2xl p-6 border border-gray-700 space-y-4">
            {/* Status Header */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                    {getStatusIcon()}
                    <span className={`text-lg font-medium capitalize ${getStatusColor()}`}>
                        {session.status}
                    </span>
                </div>
                {session.status === 'running' && (
                    <div className="text-sm text-gray-400">
                        <span className="text-gray-500">Time remaining: </span>
                        <span className="text-white font-mono">
                            {minutes.toString().padStart(2, '0')}:{seconds.toString().padStart(2, '0')}
                        </span>
                    </div>
                )}
            </div>

            {/* Session Details */}
            <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                    <span className="text-gray-500">Session ID</span>
                    <p className="font-mono text-gray-300 truncate">{session.id}</p>
                </div>
                <div>
                    <span className="text-gray-500">VM Name</span>
                    <p className="font-mono text-gray-300">{session.vmName}</p>
                </div>
                <div>
                    <span className="text-gray-500">Zone</span>
                    <p className="text-gray-300">{session.zone}</p>
                </div>
                <div>
                    <span className="text-gray-500">External IP</span>
                    <p className="font-mono text-gray-300">{session.ip || 'Pending...'}</p>
                </div>
            </div>

            {/* Access Links */}
            {session.status === 'running' && url && (
                <div className="pt-4 border-t border-gray-700 space-y-3">
                    <a
                        href={url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="flex items-center justify-center gap-2 w-full py-3 px-4 bg-gradient-to-r from-green-600 to-emerald-600 hover:from-green-500 hover:to-emerald-500 text-white rounded-xl font-medium transition-all duration-200 shadow-lg shadow-green-900/30"
                    >
                        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
                        </svg>
                        Open Android in Browser
                    </a>

                    <div className="bg-gray-900/50 rounded-xl p-4">
                        <div className="flex items-center justify-between mb-2">
                            <span className="text-gray-400 text-sm">ADB Connection</span>
                            <button
                                onClick={() => navigator.clipboard.writeText(`adb connect ${session.ip}:5555`)}
                                className="text-xs text-blue-400 hover:text-blue-300"
                            >
                                Copy
                            </button>
                        </div>
                        <code className="text-green-400 font-mono text-sm">
                            adb connect {session.ip}:5555
                        </code>
                    </div>
                </div>
            )}

            {/* Starting Message */}
            {session.status === 'starting' && (
                <div className="pt-4 border-t border-gray-700">
                    <div className="flex items-center gap-3 text-yellow-400/80">
                        <svg className="w-5 h-5 animate-spin" fill="none" viewBox="0 0 24 24">
                            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                        </svg>
                        <div>
                            <p className="font-medium">Provisioning VM...</p>
                            <p className="text-sm text-gray-500">This typically takes 2-5 minutes</p>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
