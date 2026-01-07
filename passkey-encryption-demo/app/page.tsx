'use client';

import { useEffect, useMemo, useState } from 'react';

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

const storageKeys = {
  credentialId: 'passkey-demo:credential-id',
  prfSalt: 'passkey-demo:prf-salt',
};

const browserSupportsWebAuthn = () =>
  typeof window !== 'undefined' && typeof window.PublicKeyCredential !== 'undefined';

const platformAuthenticatorIsAvailable = async () => {
  if (!browserSupportsWebAuthn()) {
    return false;
  }
  return window.PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
};

const conditionalMediationIsAvailable = async () => {
  if (!browserSupportsWebAuthn()) {
    return false;
  }
  if (!window.PublicKeyCredential.isConditionalMediationAvailable) {
    return false;
  }
  return window.PublicKeyCredential.isConditionalMediationAvailable();
};

function toBase64Url(data: ArrayBuffer) {
  const bytes = new Uint8Array(data);
  let binary = '';
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

function fromBase64Url(value: string) {
  const padded = value.replace(/-/g, '+').replace(/_/g, '/');
  const padLength = (4 - (padded.length % 4)) % 4;
  const normalized = padded + '='.repeat(padLength);
  const binary = atob(normalized);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

function randomBytes(length: number) {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return bytes.buffer;
}

function concatBuffers(...buffers: ArrayBuffer[]) {
  const totalLength = buffers.reduce((sum, buffer) => sum + buffer.byteLength, 0);
  const merged = new Uint8Array(totalLength);
  let offset = 0;
  buffers.forEach((buffer) => {
    merged.set(new Uint8Array(buffer), offset);
    offset += buffer.byteLength;
  });
  return merged.buffer;
}

async function deriveAesKey(prfOutput: ArrayBuffer) {
  const hkdfKey = await crypto.subtle.importKey(
    'raw',
    prfOutput,
    'HKDF',
    false,
    ['deriveKey'],
  );

  return crypto.subtle.deriveKey(
    {
      name: 'HKDF',
      hash: 'SHA-256',
      salt: textEncoder.encode('passkey-encryption-demo-salt'),
      info: textEncoder.encode('passkey-demo-derivation'),
    },
    hkdfKey,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
}

async function encryptPayload(key: CryptoKey, plaintext: ArrayBuffer) {
  const iv = randomBytes(12);
  const ciphertext = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv },
    key,
    plaintext,
  );
  return concatBuffers(iv, ciphertext);
}

async function decryptPayload(key: CryptoKey, payload: ArrayBuffer) {
  const bytes = new Uint8Array(payload);
  const iv = bytes.slice(0, 12).buffer;
  const ciphertext = bytes.slice(12).buffer;
  return crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ciphertext);
}

export default function Home() {
  const [supportStatus, setSupportStatus] = useState('Checking WebAuthn support...');
  const [conditionalStatus, setConditionalStatus] = useState('Checking conditional UI...');
  const [platformStatus, setPlatformStatus] = useState('Checking platform authenticator...');
  const [credentialId, setCredentialId] = useState<string | null>(null);
  const [prfSalt, setPrfSalt] = useState<ArrayBuffer | null>(null);
  const [derivedKey, setDerivedKey] = useState<CryptoKey | null>(null);
  const [passkeyReady, setPasskeyReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string[]>([]);

  const [plaintext, setPlaintext] = useState('');
  const [ciphertext, setCiphertext] = useState('');
  const [decryptedText, setDecryptedText] = useState('');

  const [fileToEncrypt, setFileToEncrypt] = useState<File | null>(null);
  const [fileToDecrypt, setFileToDecrypt] = useState<File | null>(null);
  const [encryptedDownloadUrl, setEncryptedDownloadUrl] = useState<string | null>(null);
  const [decryptedDownloadUrl, setDecryptedDownloadUrl] = useState<string | null>(null);

  useEffect(() => {
    const checkSupport = async () => {
      const supported = browserSupportsWebAuthn();
      if (!supported) {
        setSupportStatus('WebAuthn is not available in this browser.');
        setPlatformStatus('Platform authenticator unavailable.');
        setConditionalStatus('Conditional UI not available.');
        return;
      }
      setSupportStatus('WebAuthn supported.');
      const platform = await platformAuthenticatorIsAvailable();
      setPlatformStatus(
        platform
          ? 'Platform authenticator available.'
          : 'No platform authenticator detected.',
      );
      const conditional = await conditionalMediationIsAvailable();
      setConditionalStatus(
        conditional
          ? 'Conditional UI is available.'
          : 'Conditional UI is not available.',
      );
    };

    checkSupport();

    const storedCredentialId = localStorage.getItem(storageKeys.credentialId);
    const storedSalt = localStorage.getItem(storageKeys.prfSalt);
    if (storedCredentialId && storedSalt) {
      setCredentialId(storedCredentialId);
      setPrfSalt(fromBase64Url(storedSalt));
      setPasskeyReady(true);
    }
  }, []);

  useEffect(() => {
    return () => {
      if (encryptedDownloadUrl) URL.revokeObjectURL(encryptedDownloadUrl);
      if (decryptedDownloadUrl) URL.revokeObjectURL(decryptedDownloadUrl);
    };
  }, [encryptedDownloadUrl, decryptedDownloadUrl]);

  const canUseKey = useMemo(() => derivedKey !== null, [derivedKey]);

  const addStatus = (message: string) => {
    setStatus((prev) => [message, ...prev].slice(0, 5));
  };

  const clearPasskey = () => {
    localStorage.removeItem(storageKeys.credentialId);
    localStorage.removeItem(storageKeys.prfSalt);
    setCredentialId(null);
    setPrfSalt(null);
    setDerivedKey(null);
    setPasskeyReady(false);
    addStatus('Cleared stored passkey metadata.');
  };

  const registerPasskey = async () => {
    if (!browserSupportsWebAuthn()) {
      addStatus('WebAuthn not supported in this browser.');
      return;
    }

    setBusy(true);
    try {
      const userId = randomBytes(16);
      const salt = randomBytes(32);
      const publicKey: PublicKeyCredentialCreationOptions = {
        challenge: randomBytes(32),
        rp: { name: 'Passkey Encryption Demo', id: window.location.hostname },
        user: {
          id: userId,
          name: 'demo@example.com',
          displayName: 'Demo User',
        },
        pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
        authenticatorSelection: {
          residentKey: 'required',
          userVerification: 'required',
        },
        timeout: 60000,
        attestation: 'none',
        extensions: {
          prf: { eval: { first: salt } },
        },
      };

      const credential = (await navigator.credentials.create({
        publicKey,
      })) as PublicKeyCredential | null;

      if (!credential) {
        addStatus('Passkey creation was cancelled or failed.');
        return;
      }

      const extensionResults = credential.getClientExtensionResults();
      const prfEnabled = (extensionResults.prf as { enabled?: boolean })?.enabled;
      if (!prfEnabled) {
        addStatus(
          'PRF extension not enabled during registration; key derivation may fail.',
        );
      }

      const credentialIdValue = toBase64Url(credential.rawId);
      localStorage.setItem(storageKeys.credentialId, credentialIdValue);
      localStorage.setItem(storageKeys.prfSalt, toBase64Url(salt));

      setCredentialId(credentialIdValue);
      setPrfSalt(salt);
      setPasskeyReady(true);
      addStatus('Passkey created. You can now derive an encryption key.');
    } catch (error) {
      addStatus(`Passkey creation failed: ${(error as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const deriveKeyFromPasskey = async () => {
    if (!credentialId || !prfSalt) {
      addStatus('Create a passkey first to derive an encryption key.');
      return;
    }

    setBusy(true);
    try {
      const allowCredentials = [
        {
          id: fromBase64Url(credentialId),
          type: 'public-key' as const,
        },
      ];

      const publicKey: PublicKeyCredentialRequestOptions = {
        challenge: randomBytes(32),
        allowCredentials,
        userVerification: 'required',
        extensions: {
          prf: { eval: { first: prfSalt } },
        },
        timeout: 60000,
      };

      const assertion = (await navigator.credentials.get({
        publicKey,
      })) as PublicKeyCredential | null;

      if (!assertion) {
        addStatus('Authentication cancelled or failed.');
        return;
      }

      const extensionResults = assertion.getClientExtensionResults();
      const prfResult = (extensionResults.prf as {
        results?: { first?: ArrayBuffer };
      })?.results?.first;

      if (!prfResult) {
        addStatus(
          'PRF extension unavailable. Use a browser/passkey that supports PRF.',
        );
        return;
      }

      const key = await deriveAesKey(prfResult);
      setDerivedKey(key);
      addStatus('Derived AES-GCM key from passkey PRF output.');
    } catch (error) {
      addStatus(`Failed to derive key: ${(error as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const encryptText = async () => {
    if (!derivedKey) {
      addStatus('Derive an encryption key first.');
      return;
    }
    setBusy(true);
    try {
      const payload = await encryptPayload(
        derivedKey,
        textEncoder.encode(plaintext),
      );
      const encoded = toBase64Url(payload);
      setCiphertext(encoded);
      addStatus('Encrypted text with AES-GCM.');
    } catch (error) {
      addStatus(`Text encryption failed: ${(error as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const decryptText = async () => {
    if (!derivedKey || !ciphertext) {
      addStatus('Derive a key and provide ciphertext first.');
      return;
    }
    setBusy(true);
    try {
      const payload = fromBase64Url(ciphertext);
      const decrypted = await decryptPayload(derivedKey, payload);
      setDecryptedText(textDecoder.decode(decrypted));
      addStatus('Decrypted text successfully.');
    } catch (error) {
      addStatus(`Text decryption failed: ${(error as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const encryptFile = async () => {
    if (!derivedKey || !fileToEncrypt) {
      addStatus('Select a file and derive a key first.');
      return;
    }
    setBusy(true);
    try {
      const data = await fileToEncrypt.arrayBuffer();
      const encrypted = await encryptPayload(derivedKey, data);
      const blob = new Blob([encrypted], { type: 'application/octet-stream' });
      if (encryptedDownloadUrl) URL.revokeObjectURL(encryptedDownloadUrl);
      const url = URL.createObjectURL(blob);
      setEncryptedDownloadUrl(url);
      addStatus('Encrypted file ready for download.');
    } catch (error) {
      addStatus(`File encryption failed: ${(error as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const decryptFile = async () => {
    if (!derivedKey || !fileToDecrypt) {
      addStatus('Select an encrypted file and derive a key first.');
      return;
    }
    setBusy(true);
    try {
      const data = await fileToDecrypt.arrayBuffer();
      const decrypted = await decryptPayload(derivedKey, data);
      const blob = new Blob([decrypted], { type: 'application/octet-stream' });
      if (decryptedDownloadUrl) URL.revokeObjectURL(decryptedDownloadUrl);
      const url = URL.createObjectURL(blob);
      setDecryptedDownloadUrl(url);
      addStatus('Decrypted file ready for download.');
    } catch (error) {
      addStatus(`File decryption failed: ${(error as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <main>
      <h1>Passkey-derived Encryption Demo</h1>
      <p>
        This demo mirrors the approach described in the 2025 passkey encryption
        article by using the WebAuthn PRF extension to derive stable key material
        from a passkey. Use the buttons below to register a passkey, derive an
        AES-GCM key, and encrypt or decrypt text and files locally.
      </p>
      <p className="note">
        For real products, WebAuthn registration and authentication should be
        backed by a server for challenge generation and verification. This demo
        keeps everything client-side for clarity.
      </p>

      <section>
        <h2>Environment</h2>
        <div className="status">
          <div>{supportStatus}</div>
          <div>{platformStatus}</div>
          <div>{conditionalStatus}</div>
          <div>
            Stored passkey:{' '}
            {passkeyReady ? 'Found in local storage.' : 'Not registered yet.'}
          </div>
          <div>Derived key: {canUseKey ? 'Ready' : 'Not derived.'}</div>
        </div>
      </section>

      <section className="grid">
        <h2>Passkey setup</h2>
        <button onClick={registerPasskey} disabled={busy}>
          Create / Register Passkey
        </button>
        <button onClick={deriveKeyFromPasskey} disabled={busy} className="secondary">
          Derive Encryption Key
        </button>
        <button onClick={clearPasskey} disabled={busy} className="secondary">
          Clear Stored Passkey
        </button>
        <p className="note">
          The PRF extension must be supported by your passkey provider (Chrome +
          recent platform authenticators). If PRF is unavailable, the key cannot
          be derived.
        </p>
      </section>

      <section className="grid grid-two">
        <div>
          <h2>Encrypt text</h2>
          <label htmlFor="plaintext">Plaintext</label>
          <textarea
            id="plaintext"
            value={plaintext}
            onChange={(event) => setPlaintext(event.target.value)}
            placeholder="Type a message to encrypt"
          />
          <button onClick={encryptText} disabled={busy}>
            Encrypt Text
          </button>
          <label htmlFor="ciphertext">Ciphertext (base64url)</label>
          <textarea
            id="ciphertext"
            value={ciphertext}
            onChange={(event) => setCiphertext(event.target.value)}
            placeholder="Encrypted output"
          />
          <button onClick={decryptText} disabled={busy} className="secondary">
            Decrypt Text
          </button>
          <label>Decrypted output</label>
          <div className="output">{decryptedText || '—'}</div>
        </div>

        <div>
          <h2>Encrypt files</h2>
          <label htmlFor="file-encrypt">Select file to encrypt</label>
          <input
            id="file-encrypt"
            type="file"
            onChange={(event) =>
              setFileToEncrypt(event.target.files?.[0] ?? null)
            }
          />
          <button onClick={encryptFile} disabled={busy}>
            Encrypt File
          </button>
          {encryptedDownloadUrl && (
            <p>
              <a href={encryptedDownloadUrl} download="encrypted.bin">
                Download encrypted file
              </a>
            </p>
          )}

          <label htmlFor="file-decrypt">Select encrypted file</label>
          <input
            id="file-decrypt"
            type="file"
            onChange={(event) =>
              setFileToDecrypt(event.target.files?.[0] ?? null)
            }
          />
          <button onClick={decryptFile} disabled={busy} className="secondary">
            Decrypt File
          </button>
          {decryptedDownloadUrl && (
            <p>
              <a href={decryptedDownloadUrl} download="decrypted.bin">
                Download decrypted file
              </a>
            </p>
          )}
        </div>
      </section>

      <section>
        <h2>Activity log</h2>
        <div className="output">
          {status.length === 0
            ? 'Actions and errors will appear here.'
            : status.join('\n')}
        </div>
      </section>
    </main>
  );
}
