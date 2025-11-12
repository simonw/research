/**
 * Reclaim Protocol - Proof of Concept
 *
 * This POC demonstrates how to use Reclaim Protocol's zkTLS
 * to extract and verify data from a web source (e.g., LinkedIn profile)
 * with cryptographic provenance guarantees.
 *
 * Key Features:
 * - No certificate installation required
 * - Generates zero-knowledge proofs
 * - 2-4 second proof generation on mobile
 * - Works with 2500+ data sources
 */

const { ReclaimProofRequest } = require('@reclaimprotocol/js-sdk');

// Configuration
const APP_ID = 'YOUR_APP_ID'; // Get from Reclaim Protocol dashboard
const APP_SECRET = 'YOUR_APP_SECRET';
const PROVIDER_ID = 'linkedin-education'; // Example: LinkedIn education data

/**
 * Initialize Reclaim Proof Request
 */
async function initializeReclaimRequest() {
  const reclaimProofRequest = await ReclaimProofRequest.init(
    APP_ID,
    APP_SECRET,
    PROVIDER_ID
  );

  // Optional: Add custom context for the proof
  reclaimProofRequest.setContext({
    contextAddress: 'user-123',
    contextMessage: 'Requesting education history verification'
  });

  return reclaimProofRequest;
}

/**
 * Generate Request URL
 * User will open this URL to authenticate and generate proof
 */
async function generateRequestUrl(reclaimProofRequest) {
  const requestUrl = await reclaimProofRequest.getRequestUrl();
  console.log('Request URL:', requestUrl);
  console.log('\nUser should open this URL to:');
  console.log('1. Authenticate with LinkedIn');
  console.log('2. Navigate to their profile/education page');
  console.log('3. Allow data extraction');
  console.log('4. Generate zkProof');

  return requestUrl;
}

/**
 * Listen for proof submission
 * This callback is triggered when the user completes the flow
 */
async function waitForProofSubmission(reclaimProofRequest) {
  return new Promise((resolve, reject) => {
    reclaimProofRequest.startSession({
      onSuccess: (proofs) => {
        console.log('\n✓ Proof received successfully!');
        resolve(proofs);
      },
      onError: (error) => {
        console.error('✗ Error generating proof:', error);
        reject(error);
      }
    });
  });
}

/**
 * Verify the proof
 * Cryptographically verify the proof's authenticity
 */
async function verifyProof(proof) {
  try {
    const isValid = await ReclaimProofRequest.verifyProof(proof);

    if (isValid) {
      console.log('\n✓ Proof verification: VALID');
      console.log('\nExtracted Data:');
      console.log(JSON.stringify(proof.extractedData, null, 2));

      // Proof contains:
      // - extractedData: The actual data scraped from the page
      // - context: User context we set earlier
      // - signatures: Cryptographic signatures for verification
      // - timestamp: When the proof was generated

      return {
        verified: true,
        data: proof.extractedData,
        timestamp: proof.timestamp,
        provenance: proof.signatures
      };
    } else {
      console.error('✗ Proof verification: INVALID');
      return { verified: false };
    }
  } catch (error) {
    console.error('Error verifying proof:', error);
    throw error;
  }
}

/**
 * Custom data extraction using zkFetch
 * For sources not in the 2500+ pre-built providers
 */
async function customDataExtraction() {
  const { zkFetch } = require('@reclaimprotocol/zk-fetch');

  // Example: Extract data from a custom API endpoint
  const result = await zkFetch('https://api.example.com/user/profile', {
    method: 'GET',
    headers: {
      'Authorization': 'Bearer USER_TOKEN'
    },
    // Define what data to extract
    responseMatchers: [
      {
        type: 'json',
        path: '$.user.education',
        name: 'education'
      },
      {
        type: 'json',
        path: '$.user.email',
        name: 'email'
      }
    ]
  });

  console.log('Custom extraction result:', result);
  return result;
}

/**
 * Main execution flow
 */
async function main() {
  try {
    console.log('=== Reclaim Protocol zkTLS POC ===\n');

    // Step 1: Initialize request
    console.log('Step 1: Initializing Reclaim proof request...');
    const reclaimProofRequest = await initializeReclaimRequest();

    // Step 2: Generate URL for user
    console.log('\nStep 2: Generating request URL...');
    const requestUrl = await generateRequestUrl(reclaimProofRequest);

    // Step 3: Wait for user to complete the flow
    console.log('\nStep 3: Waiting for proof submission...');
    console.log('(In production, user would open URL in mobile app/browser)');
    const proofs = await waitForProofSubmission(reclaimProofRequest);

    // Step 4: Verify the proof
    console.log('\nStep 4: Verifying proof...');
    const verificationResult = await verifyProof(proofs[0]);

    if (verificationResult.verified) {
      console.log('\n=== Success! ===');
      console.log('Data extracted and verified with cryptographic proof');
      console.log('Provenance guaranteed via zkTLS');
    }

  } catch (error) {
    console.error('Error in main flow:', error);
  }
}

/**
 * Example: Mobile SDK usage (React Native)
 */
function mobileSDKExample() {
  /*
  import { ReclaimSDK } from '@reclaimprotocol/react-native-sdk';

  const reclaim = new ReclaimSDK(APP_ID, APP_SECRET);

  // In your React Native component:
  const handleDataExtraction = async () => {
    try {
      const proof = await reclaim.requestProof({
        providerId: 'linkedin-education',
        context: 'Education verification for loan application'
      });

      // Proof is automatically generated on the device
      // No external browser needed

      if (proof.verified) {
        console.log('Education data:', proof.data);
      }
    } catch (error) {
      console.error('Error:', error);
    }
  };
  */
}

// Run if executed directly
if (require.main === module) {
  main();
}

module.exports = {
  initializeReclaimRequest,
  generateRequestUrl,
  waitForProofSubmission,
  verifyProof,
  customDataExtraction
};
