'use client';

import React, { useEffect, useState, useCallback } from 'react';
import { useSession } from '@/hooks/useSession';
import { ProviderConfig } from '@/lib/providers';
import { FlowHeader } from './FlowHeader';
import { DynamicForm } from './DynamicForm';
import { BrowserViewer } from '../BrowserViewer';
import { AlertCircle, CheckCircle2, Loader2, Info, ChevronDown, ChevronUp, Shield } from 'lucide-react';
import { useRouter } from 'next/navigation';

interface ConnectionFlowProps {
  provider: ProviderConfig;
}

type FlowStatus = 'intro' | 'explainer' | 'initializing' | 'input' | 'processing' | 'success' | 'error';

export function ConnectionFlow({ provider }: ConnectionFlowProps) {
  const session = useSession();
  const router = useRouter();
  const [status, setStatus] = useState<FlowStatus>('intro');
  const [showDebug, setShowDebug] = useState(false);
  const [isFormSubmitting, setIsFormSubmitting] = useState(false);

  const startConnection = useCallback(async () => {
    setStatus('initializing');
    try {
      await session.createSession();
    } catch (e) {
      setStatus('error');
    }
  }, [session]);

  useEffect(() => {
    if (session.connected && session.status === 'idle' && session.sessionId && status === 'initializing') {
      const runAutomationScript = async () => {
        try {
          const response = await fetch(`/automations/${provider.script}.js`);
          if (!response.ok) throw new Error('Script not found');
          const code = await response.text();
          await session.runScript(code);
        } catch (e) {
          setStatus('error');
        }
      };
      runAutomationScript();
    }
  }, [session.connected, session.status, session.sessionId, status, provider.script, session.runScript]);

  useEffect(() => {
    if (session.inputRequest) {
      setStatus('input');
      setIsFormSubmitting(false);
    } else if (session.status === 'running') {
      setStatus('processing');
    } else if (session.status === 'done') {
      setStatus('success');
    } else if (session.status === 'error') {
      setStatus('error');
    }
  }, [session.inputRequest, session.status]);

  const handleFormSubmit = useCallback((values: Record<string, unknown>) => {
    setIsFormSubmitting(true);
    session.submitInput(values);
  }, [session]);

  const handleCancel = useCallback(async () => {
    await session.destroySession();
    router.push('/');
  }, [session, router]);

  const handleClose = useCallback(async () => {
    await session.destroySession();
    router.push('/');
  }, [session, router]);

  const renderContent = () => {
    switch (status) {
      case 'intro':
        return (
          <div className="flex flex-col items-center py-6 text-center animate-in fade-in slide-in-from-bottom-4 duration-500">
            <div className="w-20 h-20 bg-accent/10 rounded-3xl flex items-center justify-center mb-8 shadow-inner border border-accent/20">
              <Shield className="w-10 h-10 text-accent" />
            </div>
            <h2 className="text-2xl font-bold text-foreground mb-4">Link your {provider.name} account</h2>
            <p className="text-sm text-text-secondary max-w-xs mb-10 leading-relaxed">
              Cloudflare Remote Browser uses a secure automation process to link your {provider.name} account.
            </p>
            <button
              onClick={() => setStatus('explainer')}
              className="w-full py-4 bg-foreground text-background font-bold rounded-xl hover:opacity-90 transition-all shadow-lg active:scale-[0.98]"
            >
              Continue
            </button>
          </div>
        );

      case 'explainer':
        return (
          <div className="flex flex-col py-2 animate-in fade-in slide-in-from-right-4 duration-500">
            <h2 className="text-2xl font-bold text-foreground mb-8">How it works</h2>
            <div className="space-y-8 mb-12">
              <div className="flex items-start gap-4">
                <div className="w-10 h-10 rounded-full bg-blue-50 flex items-center justify-center shrink-0">
                  <Shield size={20} className="text-blue-500" />
                </div>
                <div>
                  <h4 className="font-bold text-foreground mb-1 text-sm">Secure Connection</h4>
                  <p className="text-xs text-text-secondary leading-relaxed">
                    We use a remote browser environment to authenticate directly with {provider.name}.
                  </p>
                </div>
              </div>
              <div className="flex items-start gap-4">
                <div className="w-10 h-10 rounded-full bg-green-50 flex items-center justify-center shrink-0">
                  <CheckCircle2 size={20} className="text-green-500" />
                </div>
                <div>
                  <h4 className="font-bold text-foreground mb-1 text-sm">You're in Control</h4>
                  <p className="text-xs text-text-secondary leading-relaxed">
                    Only the data you see on the screen will be imported. Your credentials are never stored.
                  </p>
                </div>
              </div>
              <div className="flex items-start gap-4">
                <div className="w-10 h-10 rounded-full bg-purple-50 flex items-center justify-center shrink-0">
                  <Info size={20} className="text-purple-500" />
                </div>
                <div>
                  <h4 className="font-bold text-foreground mb-1 text-sm">Transparent Process</h4>
                  <p className="text-xs text-text-secondary leading-relaxed">
                    You can monitor the automation process through our debug view if you choose.
                  </p>
                </div>
              </div>
            </div>
            <button
              onClick={startConnection}
              className="w-full py-4 bg-foreground text-background font-bold rounded-xl hover:opacity-90 transition-all shadow-lg active:scale-[0.98]"
            >
              Continue
            </button>
          </div>
        );

      case 'initializing':
        return (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <Loader2 className="w-12 h-12 text-accent animate-spin mb-4" />
            <h3 className="text-lg font-semibold text-foreground mb-2">Connecting to {provider.name}</h3>
            <p className="text-sm text-text-secondary max-w-xs">
              Securely launching a browser session to authenticate with {provider.name}.
            </p>
          </div>
        );

      case 'input':
        if (!session.inputRequest) return null;
        return (
          <div className="py-2 animate-in fade-in slide-in-from-right-4 duration-500">
            <div className="flex flex-col items-center mb-10">
              <div className="w-16 h-16 bg-surface border border-border rounded-2xl shadow-sm flex items-center justify-center mb-4">
                <img src={provider.logo} alt={provider.name} className="w-10 h-10 object-contain" />
              </div>
              <h3 className="text-xl font-bold text-foreground mb-1">Enter credentials</h3>
              <p className="text-xs text-text-tertiary font-medium uppercase tracking-widest">To connect with {provider.name}</p>
            </div>
            <DynamicForm 
              input={session.inputRequest.input} 
              onSubmit={handleFormSubmit} 
              onCancel={handleCancel}
              isSubmitting={isFormSubmitting}
            />
          </div>
        );

      case 'processing':
        return (
          <div className="flex flex-col items-center justify-center py-16 text-center animate-in fade-in duration-500">
            <div className="relative mb-10 scale-125">
              <Loader2 className="w-20 h-20 text-accent animate-spin" strokeWidth={1.5} />
              <div className="absolute inset-0 flex items-center justify-center">
                <img src={provider.logo} alt="" className="w-8 h-8 rounded-lg shadow-sm border border-border" />
              </div>
            </div>
            <h3 className="text-xl font-bold text-foreground mb-3">
              {session.automationData.status as string || 'Connecting...'}
            </h3>
            <p className="text-sm text-text-secondary max-w-[240px] leading-relaxed">
              This may take a moment. We're establishing a secure connection to {provider.name}.
            </p>
          </div>
        );

      case 'success':
        return (
          <div className="flex flex-col items-center justify-center py-10 text-center animate-in fade-in zoom-in-95 duration-500">
            <div className="w-24 h-24 bg-success/10 rounded-full flex items-center justify-center mb-10 shadow-inner">
              <CheckCircle2 className="w-12 h-12 text-success" />
            </div>
            <h3 className="text-2xl font-bold text-foreground mb-4">Linked Successfully</h3>
            <p className="text-sm text-text-secondary max-w-xs mb-10 leading-relaxed">
              Your {provider.name} account is now connected to Cloudflare Remote Browser.
            </p>
            
            <div className="w-full p-5 bg-background border border-border rounded-2xl text-left mb-10 overflow-hidden shadow-sm">
              <p className="text-[10px] font-bold text-text-tertiary uppercase tracking-wider mb-3 px-1">Imported Profile</p>
              <div className="space-y-1 max-h-40 overflow-y-auto pr-2 custom-scrollbar">
                <pre className="text-[11px] text-foreground font-mono leading-tight">
                  {JSON.stringify(session.result, null, 2)}
                </pre>
              </div>
            </div>

            <button
              onClick={handleClose}
              className="w-full py-4 bg-foreground text-background font-bold rounded-xl hover:opacity-90 transition-all shadow-lg active:scale-[0.98]"
            >
              Continue
            </button>
          </div>
        );

      case 'error':
        return (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <div className="w-16 h-16 bg-error/10 rounded-full flex items-center justify-center mb-6">
              <AlertCircle className="w-10 h-10 text-error" />
            </div>
            <h3 className="text-xl font-bold text-foreground mb-2">Connection Failed</h3>
            <p className="text-sm text-text-secondary max-w-xs mb-8">
              {session.error || 'Something went wrong while connecting to your account. Please try again.'}
            </p>
            <div className="flex flex-col w-full gap-3">
              <button
                onClick={() => window.location.reload()}
                className="w-full py-3 bg-accent text-white font-semibold rounded-lg hover:bg-accent-hover transition-all"
              >
                Try Again
              </button>
              <button
                onClick={handleClose}
                className="w-full py-3 text-text-secondary font-semibold hover:text-foreground transition-all"
              >
                Cancel
              </button>
            </div>
          </div>
        );
    }
  };

  return (
    <div className="min-h-screen bg-background flex flex-col lg:flex-row items-center justify-center p-4 gap-6 lg:gap-12 transition-all duration-500">
      <div className="w-full max-w-md bg-surface border border-border rounded-2xl shadow-2xl overflow-hidden flex flex-col shrink-0">
        <FlowHeader providerName={provider.name} onClose={handleClose} />
        
        <div className="px-8 py-10 flex-1 overflow-y-auto min-h-[400px]">
          {renderContent()}
        </div>

        {provider.privacyNote && status !== 'success' && status !== 'error' && (
          <div className="px-8 py-4 bg-background/50 border-t border-border flex items-start gap-3">
            <Info size={14} className="text-text-tertiary mt-0.5 shrink-0" />
            <p className="text-[11px] text-text-tertiary leading-normal">
              {provider.privacyNote}
            </p>
          </div>
        )}
      </div>

      <div className={`w-full transition-all duration-500 ${showDebug ? 'max-w-md lg:max-w-2xl' : 'max-w-md'}`}>
        <button
          onClick={() => setShowDebug(!showDebug)}
          className="w-full py-2 flex items-center justify-between px-4 text-xs font-semibold text-text-tertiary hover:text-foreground transition-colors uppercase tracking-widest"
        >
          <span>Debug View</span>
          {showDebug ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
        </button>
        
        {showDebug && (
          <div className="mt-4 bg-surface border border-border rounded-xl overflow-hidden shadow-lg animate-in fade-in slide-in-from-top-2 lg:slide-in-from-left-2 duration-500">
            <div className="p-3 bg-background border-b border-border flex items-center justify-between">
              <span className="text-[10px] font-mono text-text-tertiary">REMOTE BROWSER STREAM</span>
              <div className="flex items-center gap-1.5">
                <div className={`w-1.5 h-1.5 rounded-full ${session.connected ? 'bg-success' : 'bg-error'}`} />
                <span className="text-[10px] text-text-tertiary uppercase">{session.connected ? 'Live' : 'Offline'}</span>
              </div>
            </div>
            <div className="aspect-video bg-black flex items-center justify-center">
              <BrowserViewer
                onFrame={session.setOnFrame}
                sendInput={session.sendInput}
                status={session.status}
                viewport={session.viewport}
                takeoverMessage={session.takeoverMessage}
                agentCursor={session.agentCursor}
              />
            </div>
            <div className="p-3 bg-background border-t border-border flex flex-col gap-1">
              <div className="flex justify-between text-[10px] font-mono text-text-tertiary uppercase">
                <span>Status: {session.status}</span>
                <span>Session: {session.sessionId?.slice(0, 8) || 'None'}</span>
              </div>
              {typeof session.automationData.status === 'string' && (
                <div className="text-[10px] font-mono text-accent truncate">
                  LOG: {session.automationData.status}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
