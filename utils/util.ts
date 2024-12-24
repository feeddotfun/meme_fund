import { PublicKey, Commitment } from "@solana/web3.js";

export const DEFAULT_COMMITMENT: Commitment = "finalized";
export const GLOBAL_ACCOUNT_SEED = "global";
export const PUMP_PROGRAM_ID = new PublicKey('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P');
export const MPL_TOKEN_METADATA = new PublicKey('metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s');
export const EVENT_AUTHORITY = new PublicKey('Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1');
export const PUMP_FEE_RECIPIENT = new PublicKey("68yFSZxzLWJXkxxRGydZ63C6mHx1NLEDWmwN9Lb5yySg");
export function uuidToMemeIdAndBuffer(uuid: string): { memeId: number[], buffer: Buffer } {
    const hexString = uuid.replace(/-/g, '');
    const buffer = Buffer.from(hexString, 'hex');
    const memeId = Array.from(buffer);
    return { memeId, buffer };
}