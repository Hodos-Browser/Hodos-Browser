import type { AddressData } from './address';
import type { TransactionResponse, BroadcastResponse } from './transaction';
import type { HistoryEntry, HistorySearchParams, HistoryGetParams, ClearRangeParams } from './history';

declare global {
  interface Window {
    hodosBrowser: {
      history: {
        get: (params?: HistoryGetParams) => HistoryEntry[];
        search: (params: HistorySearchParams) => HistoryEntry[];
        delete: (url: string) => boolean;
        clearAll: () => boolean;
        clearRange: (params: ClearRangeParams) => boolean;
      };
      wallet: {
        getStatus: () => Promise<{ exists: boolean; needsBackup: boolean }>;
        create: () => Promise<{ success: boolean; wallet?: { mnemonic: string; address?: string; version?: string; backedUp?: boolean }; error?: string }>;
        load: () => Promise<{ success: boolean; address: string; mnemonic: string; version: string; backedUp: boolean }>;
        getInfo: () => Promise<{ version: string; mnemonic: string; address: string; backedUp: boolean }>;
        generateAddress: () => Promise<AddressData>;
        getCurrentAddress: () => Promise<AddressData>;
        getAddresses: () => Promise<AddressData[]>;
        markBackedUp: () => Promise<{ success: boolean }>;
        getBackupModalState: () => Promise<{ shown: boolean }>;
        setBackupModalState: (shown: boolean) => Promise<{ success: boolean }>;
        getBalance: () => Promise<{ balance: number }>;
        sendTransaction: (data: { recipient: string; amount: number }) => Promise<TransactionResponse>;
        getTransactionHistory: () => Promise<any[]>;
      };
      address: {
        generate: () => Promise<AddressData>;
      };
      navigation: {
        navigate: (path: string) => void;
      };
      overlay: {
        show: () => void;
        hide: () => void;
        toggleInput: (enable: boolean) => void;
        close: () => void;
      };
      overlayPanel: {
        open: (panelName: string) => void;
        toggleInput: (enable: boolean) => void;
      };
      omnibox: {
        show: (query: string) => void;
        hide: () => void;
        createOrShow: () => void;
        getSuggestions: (query: string) => Promise<any[]>;
      };
    };
    cefMessage?: {
      send: (channel: string, args: any[]) => void;
    };
    triggerPanel?: (panelName: string) => void;
    onAddressGenerated?: (data: AddressData) => void;
    onAddressError?: (error: string) => void;
    onSendTransactionResponse?: (data: TransactionResponse) => void;
    onSendTransactionError?: (error: string) => void;
    onGetBalanceResponse?: (data: { balance: number }) => void;
    onGetBalanceError?: (error: string) => void;
    onGetTransactionHistoryResponse?: (data: any[]) => void;
    onGetTransactionHistoryError?: (error: string) => void;
    onWalletStatusResponse?: (data: { exists: boolean; needsBackup: boolean }) => void;
    onWalletStatusError?: (error: string) => void;
    onCreateWalletResponse?: (data: { success: boolean; mnemonic: string; address: string; version: string }) => void;
    onCreateWalletError?: (error: string) => void;
    onLoadWalletResponse?: (data: { success: boolean; address: string; mnemonic: string; version: string; backedUp: boolean }) => void;
    onLoadWalletError?: (error: string) => void;
    onGetWalletInfoResponse?: (data: { version: string; mnemonic: string; address: string; backedUp: boolean }) => void;
    onGetWalletInfoError?: (error: string) => void;
    onGetCurrentAddressResponse?: (data: AddressData) => void;
    onGetCurrentAddressError?: (error: string) => void;
    onGetAddressesResponse?: (data: AddressData[]) => void;
    onGetAddressesError?: (error: string) => void;
    onMarkWalletBackedUpResponse?: (data: { success: boolean }) => void;
    onMarkWalletBackedUpError?: (error: string) => void;
    onGetBackupModalStateResponse?: (data: { shown: boolean }) => void;
    onSetBackupModalStateResponse?: (data: { success: boolean }) => void;
    allSystemsReady?: boolean;
     __overlayReady?: boolean;
  }
}

export {};
