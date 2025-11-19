export type IdentityData = {
  publicKey: string;
  privateKey: string;
  address: string;
  backedUp: boolean;
};

export type BackupCheck = {
  backedUp: true;
};

export type IdentityResult = IdentityData | BackupCheck;
