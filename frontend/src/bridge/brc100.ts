/**
 * BRC-100 Bridge for Bitcoin Browser
 * Provides TypeScript interface to BRC-100 functionality
 */

// BRC-100 Types
export interface BRC100Status {
  available: boolean;
  version: string;
  features: string[];
}

export interface IdentityData {
  issuer: string;
  subject: string;
  publicKey: string;
  timestamp: string;
  certificate?: string;
}

export interface AuthChallengeRequest {
  appId: string;
  purpose: string;
  timestamp?: string;
}

export interface AuthChallenge {
  challenge: string;
  timestamp: string;
  expiresAt: string;
}

export interface AuthRequest {
  challenge: string;
  response: string;
  signature?: string;
}

export interface AuthResponse {
  success: boolean;
  sessionId?: string;
  expiresAt?: string;
  error?: string;
}

export interface SessionData {
  appId: string;
  sessionId: string;
  expiresAt: string;
  permissions: string[];
}

export interface BEEFAction {
  type: string;
  data: any;
  identity?: string;
  timestamp: string;
}

export interface BEEFTransaction {
  sessionId: string;
  appDomain: string;
  actions: BEEFAction[];
  identity?: IdentityData;
  timestamp: string;
  spvData?: SPVData;
}

export interface SPVData {
  merkleProofs: MerkleProof[];
  blockHeaders: BlockHeader[];
  transactionData: TransactionData[];
  identityProofs: IdentityProof[];
}

export interface MerkleProof {
  blockHeight: number;
  path: MerklePath[];
}

export interface MerklePath {
  hash: string;
  offset: number;
}

export interface BlockHeader {
  hash: string;
  height: number;
  merkleRoot: string;
  timestamp: string;
  previousHash: string;
  nonce: number;
  bits: number;
}

export interface TransactionData {
  txid: string;
  hash: string;
  blockHeight: number;
  confirmations: number;
  size: number;
  fee: number;
  timestamp: string;
  inputs: InputData[];
  outputs: OutputData[];
}

export interface InputData {
  prevOutHash: string;
  prevOutIndex: number;
  scriptSig: string;
  sequence: number;
}

export interface OutputData {
  value: number;
  scriptPubKey: string;
}

export interface IdentityProof {
  identityData: IdentityData;
  merkleProof: MerkleProof;
  timestamp: string;
  transactionId: string;
}

export interface SPVVerificationRequest {
  issuer: string;
  subject: string;
  transactionId: string;
  purpose: string;
  timestamp: string;
}

export interface SPVVerificationResponse {
  valid: boolean;
  verified: boolean;
  verificationTime: string;
  result?: {
    identityProof: IdentityProof;
    valid: boolean;
  };
}

// API Response wrapper
interface APIResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

/**
 * BRC-100 Bridge Class
 * Provides methods to interact with BRC-100 functionality
 */
export class BRC100Bridge {
  private static instance: BRC100Bridge;

  private constructor() {
    // Ensure the global bitcoinBrowser.brc100 object exists
    if (typeof window !== 'undefined') {
      this.initializeGlobalObject();
    }
  }

  public static getInstance(): BRC100Bridge {
    if (!BRC100Bridge.instance) {
      BRC100Bridge.instance = new BRC100Bridge();
    }
    return BRC100Bridge.instance;
  }

  private initializeGlobalObject() {
    if (!window.bitcoinBrowser) {
      (window as any).bitcoinBrowser = {};
    }
    if (!window.bitcoinBrowser.brc100) {
      console.warn('BRC-100 API not available in bitcoinBrowser.brc100');
    }
  }

  // Status & Detection
  async status(): Promise<BRC100Status> {
    try {
      if (!window.bitcoinBrowser?.brc100) {
        throw new Error('BRC-100 API not available');
      }

      const response = await this.callNativeMethod('status');
      return response.data || response;
    } catch (error) {
      console.error('BRC-100 status check failed:', error);
      throw error;
    }
  }

  async isAvailable(): Promise<boolean> {
    try {
      if (!window.bitcoinBrowser?.brc100) {
        return false;
      }

      const response = await this.callNativeMethod('isAvailable');
      return response.data || response;
    } catch (error) {
      console.error('BRC-100 availability check failed:', error);
      return false;
    }
  }

  // Identity Management
  async generateIdentity(identityData: Partial<IdentityData>): Promise<APIResponse<IdentityData>> {
    return this.callNativeMethod('generateIdentity', identityData);
  }

  async validateIdentity(identityData: IdentityData): Promise<APIResponse<boolean>> {
    return this.callNativeMethod('validateIdentity', identityData);
  }

  async selectiveDisclosure(disclosureData: {
    identity: IdentityData;
    fields: string[];
    purpose: string;
  }): Promise<APIResponse<IdentityData>> {
    return this.callNativeMethod('selectiveDisclosure', disclosureData);
  }

  // Authentication
  async generateChallenge(request: AuthChallengeRequest): Promise<APIResponse<AuthChallenge>> {
    return this.callNativeMethod('generateChallenge', request);
  }

  async authenticate(request: AuthRequest): Promise<APIResponse<AuthResponse>> {
    return this.callNativeMethod('authenticate', request);
  }

  async deriveType42Keys(keyData: {
    sessionId: string;
    purpose: string;
    timestamp: string;
  }): Promise<APIResponse<{ publicKey: string; privateKey: string }>> {
    return this.callNativeMethod('deriveType42Keys', keyData);
  }

  // Session Management
  async createSession(sessionData: Partial<SessionData>): Promise<APIResponse<SessionData>> {
    return this.callNativeMethod('createSession', sessionData);
  }

  async validateSession(sessionData: { sessionId: string }): Promise<APIResponse<boolean>> {
    return this.callNativeMethod('validateSession', sessionData);
  }

  async revokeSession(sessionData: { sessionId: string }): Promise<APIResponse<boolean>> {
    return this.callNativeMethod('revokeSession', sessionData);
  }

  // BEEF Transaction Management
  async createBEEF(beefData: {
    actions: BEEFAction[];
    sessionId?: string;
    appDomain?: string;
    includeSPVData?: boolean;
  }): Promise<APIResponse<BEEFTransaction>> {
    return this.callNativeMethod('createBEEF', beefData);
  }

  async verifyBEEF(beefData: { beefTransaction: BEEFTransaction }): Promise<APIResponse<boolean>> {
    return this.callNativeMethod('verifyBEEF', beefData);
  }

  async broadcastBEEF(beefData: { beefTransaction: BEEFTransaction }): Promise<APIResponse<{ txid: string; success: boolean }>> {
    return this.callNativeMethod('broadcastBEEF', beefData);
  }

  // SPV Operations
  async verifySPV(spvData: SPVVerificationRequest): Promise<APIResponse<SPVVerificationResponse>> {
    return this.callNativeMethod('verifySPV', spvData);
  }

  async createSPVProof(proofData: {
    transactionId: string;
    issuer: string;
    subject: string;
    purpose: string;
    timestamp: string;
  }): Promise<APIResponse<IdentityProof>> {
    return this.callNativeMethod('createSPVProof', proofData);
  }

  // Helper method to call native C++ methods
  private async callNativeMethod(methodName: string, data?: any): Promise<any> {
    if (!window.bitcoinBrowser?.brc100) {
      throw new Error('BRC-100 API not available');
    }

    const brc100Api = window.bitcoinBrowser.brc100 as any;
    if (!brc100Api[methodName]) {
      throw new Error(`BRC-100 method '${methodName}' not available`);
    }

    try {
      const result = await brc100Api[methodName](data);
      return result;
    } catch (error) {
      console.error(`BRC-100 method '${methodName}' failed:`, error);
      throw error;
    }
  }

  // Utility methods for common workflows
  async requestAuthentication(appId: string, purpose: string): Promise<AuthResponse> {
    try {
      // Generate challenge
      const challengeResponse = await this.generateChallenge({
        appId,
        purpose,
        timestamp: new Date().toISOString()
      });

      if (!challengeResponse.success || !challengeResponse.data) {
        throw new Error('Failed to generate authentication challenge');
      }

      // Show approval modal (this would be implemented in the UI layer)
      const userApproval = await this.showAuthApprovalModal({
        appId,
        purpose,
        challenge: challengeResponse.data.challenge
      });

      if (!userApproval) {
        throw new Error('User rejected authentication request');
      }

      // Authenticate with user approval
      const authResponse = await this.authenticate({
        challenge: challengeResponse.data.challenge,
        response: 'approved'
      });

      return authResponse.data || authResponse;
    } catch (error) {
      console.error('Authentication workflow failed:', error);
      throw error;
    }
  }

  async createAndBroadcastBEEFTransaction(actions: BEEFAction[], appDomain: string = 'babbage-browser.app'): Promise<{ txid: string; success: boolean }> {
    try {
      // Create BEEF transaction
      const beefResponse = await this.createBEEF({
        actions,
        sessionId: `session_${Date.now()}`,
        appDomain,
        includeSPVData: true
      });

      if (!beefResponse.success || !beefResponse.data) {
        throw new Error('Failed to create BEEF transaction');
      }

      // Show transaction approval modal
      const userApproval = await this.showTransactionApprovalModal(beefResponse.data);

      if (!userApproval) {
        throw new Error('User rejected transaction');
      }

      // Broadcast BEEF transaction
      const broadcastResponse = await this.broadcastBEEF({
        beefTransaction: beefResponse.data
      });

      const result = broadcastResponse.data || broadcastResponse;

      // Handle both direct response and wrapped response formats
      if ('txid' in result && 'success' in result) {
        return result as { txid: string; success: boolean };
      } else if ('data' in result) {
        const data = result.data as any;
        return {
          txid: data?.txid || '',
          success: data?.success || false
        };
      } else {
        return {
          txid: '',
          success: false
        };
      }
    } catch (error) {
      console.error('BEEF transaction workflow failed:', error);
      throw error;
    }
  }

  // Placeholder methods for UI integration
  private async showAuthApprovalModal(request: {
    appId: string;
    purpose: string;
    challenge: string;
  }): Promise<boolean> {
    // This would be implemented by the UI layer
    // For now, we'll return true to allow testing
    console.log('Auth approval modal would be shown:', request);
    return true;
  }

  private async showTransactionApprovalModal(transaction: BEEFTransaction): Promise<boolean> {
    // This would be implemented by the UI layer
    // For now, we'll return true to allow testing
    console.log('Transaction approval modal would be shown:', transaction);
    return true;
  }
}

// Global type declarations
declare global {
  interface Window {
    bitcoinBrowser: {
      brc100: {
        status(): Promise<BRC100Status>;
        isAvailable(): Promise<boolean>;
        generateIdentity(data: any): Promise<any>;
        validateIdentity(data: any): Promise<any>;
        selectiveDisclosure(data: any): Promise<any>;
        generateChallenge(data: any): Promise<any>;
        authenticate(data: any): Promise<any>;
        deriveType42Keys(data: any): Promise<any>;
        createSession(data: any): Promise<any>;
        validateSession(data: any): Promise<any>;
        revokeSession(data: any): Promise<any>;
        createBEEF(data: any): Promise<any>;
        verifyBEEF(data: any): Promise<any>;
        broadcastBEEF(data: any): Promise<any>;
        verifySPV(data: any): Promise<any>;
        createSPVProof(data: any): Promise<any>;
      };
      wallet?: {
        getStatus(): Promise<any>;
        create(): Promise<any>;
        load(): Promise<any>;
        getInfo(): Promise<any>;
        generateAddress(): Promise<any>;
        getAddresses(): Promise<any>;
        getCurrentAddress(): Promise<any>;
        getBalance(): Promise<any>;
        markBackedUp(): Promise<any>;
        getBackupModalState(): Promise<any>;
        setBackupModalState(data: any): Promise<any>;
        sendTransaction(data: any): Promise<any>;
        getTransactionHistory(): Promise<any>;
      };
      identity?: any;
      navigation?: any;
      address?: any;
      overlay?: any;
    };
    cefMessage?: {
      send(message: string, ...args: any[]): void;
    };
    allSystemsReady?: boolean;
  }
}

// Export singleton instance
export const brc100 = BRC100Bridge.getInstance();
