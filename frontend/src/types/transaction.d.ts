export interface TransactionData {
  recipient: string;
  amount: string;
  feeRate: string;
  memo?: string;
}

export interface Transaction {
  txid: string;
  status: 'pending' | 'confirmed' | 'failed';
  amount: number;
  recipient: string;
  timestamp: number;
  confirmations: number;
  fee: number;
  memo?: string;
}

export interface TransactionResponse {
  txid: string;
  rawTx?: string;
  fee?: number;
  status?: string;
  broadcasted?: boolean;
  success?: boolean;
  message?: string;
  whatsOnChainUrl?: string;
}

export interface BroadcastResponse {
  txid: string;
  success: boolean;
  miners: Record<string, string>;
}

export interface BalanceData {
  balance: number;
  usdValue: number;
  isLoading: boolean;
}
