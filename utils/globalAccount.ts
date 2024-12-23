import { PublicKey } from "@solana/web3.js";
import * as anchor from "@project-serum/anchor";
import { struct, bool, u64, publicKey } from "@coral-xyz/borsh";

export class GlobalAccount {
  public discriminator: anchor.BN;
  public initialized: boolean = false;
  public authority: PublicKey;
  public feeRecipient: PublicKey;
  public initialVirtualTokenReserves: anchor.BN;
  public initialVirtualSolReserves: anchor.BN;
  public initialRealTokenReserves: anchor.BN;
  public tokenTotalSupply: anchor.BN;
  public feeBasisPoints: anchor.BN;

  constructor(
    discriminator: anchor.BN,
    initialized: boolean,
    authority: PublicKey,
    feeRecipient: PublicKey,
    initialVirtualTokenReserves: anchor.BN,
    initialVirtualSolReserves: anchor.BN,
    initialRealTokenReserves: anchor.BN,
    tokenTotalSupply: anchor.BN,
    feeBasisPoints: anchor.BN
  ) {
    this.discriminator = discriminator;
    this.initialized = initialized;
    this.authority = authority;
    this.feeRecipient = feeRecipient;
    this.initialVirtualTokenReserves = initialVirtualTokenReserves;
    this.initialVirtualSolReserves = initialVirtualSolReserves;
    this.initialRealTokenReserves = initialRealTokenReserves;
    this.tokenTotalSupply = tokenTotalSupply;
    this.feeBasisPoints = feeBasisPoints;
  }

  getInitialBuyPrice(amount: anchor.BN): anchor.BN {
    if (amount.lte(new anchor.BN(0))) {
      return new anchor.BN(0);
    }

    let n = this.initialVirtualSolReserves.mul(this.initialVirtualTokenReserves);
    let i = this.initialVirtualSolReserves.add(amount);
    let r = n.div(i).add(new anchor.BN(1));
    let s = this.initialVirtualTokenReserves.sub(r);
    return s < this.initialRealTokenReserves ? s : this.initialRealTokenReserves;
  }

  public static fromBuffer(buffer: Buffer): GlobalAccount {
    const structure = struct([
      u64("discriminator"),
      bool("initialized"),
      publicKey("authority"),
      publicKey("feeRecipient"),
      u64("initialVirtualTokenReserves"),
      u64("initialVirtualSolReserves"),
      u64("initialRealTokenReserves"),
      u64("tokenTotalSupply"),
      u64("feeBasisPoints"),
    ]);

    let value = structure.decode(buffer);
    return new GlobalAccount(
      new anchor.BN(value.discriminator.toString()),
      value.initialized,
      value.authority,
      value.feeRecipient,
      new anchor.BN(value.initialVirtualTokenReserves.toString()),
      new anchor.BN(value.initialVirtualSolReserves.toString()),
      new anchor.BN(value.initialRealTokenReserves.toString()),
      new anchor.BN(value.tokenTotalSupply.toString()),
      new anchor.BN(value.feeBasisPoints.toString())
    );
  }
}