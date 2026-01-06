'use client';

import React, { use } from 'react';
import { providers } from '@/lib/providers';
import { ConnectionFlow } from '@/components/connect/ConnectionFlow';
import { notFound } from 'next/navigation';

export default function ConnectPage({ params }: { params: Promise<{ provider: string }> }) {
  const resolvedParams = use(params);
  const provider = providers[resolvedParams.provider];
  
  if (!provider) {
    notFound();
  }
  
  return <ConnectionFlow provider={provider} />;
}
