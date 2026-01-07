import './globals.css';
import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Passkey Encryption Demo',
  description: 'Derive encryption keys from passkeys with WebAuthn PRF.',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
