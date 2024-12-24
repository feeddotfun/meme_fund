import { Connection, Keypair, PublicKey, sendAndConfirmTransaction, SystemProgram, Transaction, TransactionInstruction } from '@solana/web3.js';
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, getAssociatedTokenAddress, ASSOCIATED_TOKEN_PROGRAM_ID, getAccount } from '@solana/spl-token';
import { MemeFund } from "../target/types/meme_fund";
import { assert } from 'chai';
import { v4 as uuidv4 } from 'uuid';
import { EVENT_AUTHORITY, MPL_TOKEN_METADATA, PUMP_FEE_RECIPIENT, PUMP_PROGRAM_ID, uuidToMemeIdAndBuffer } from '../utils/util';
import { GlobalAccount } from '../utils/globalAccount';

describe("meme_fund_localnet", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    
    const program = anchor.workspace.MemeFund as Program<MemeFund>;    

    // Test constants
    const feeRecipientKey = Keypair.generate();
    const contributorKeys = Array(4).fill(null).map(() => Keypair.generate());
    const SLIPPAGE_BASIS_POINTS = new anchor.BN(100); // 5%
    const name = "Test Token";
    const symbol = "TEST";
    const uri = "https://test.uri";
    const memeUUid = uuidv4();
    const { memeId, buffer: memeIdBuffer } = uuidToMemeIdAndBuffer(memeUUid);
    const mint = Keypair.generate();
    

    // PDAs
    let registryPda: PublicKey;
    let vaultPda: PublicKey;
    let statePDA: PublicKey;
    
    before(async () => {
      await new Promise(resolve => setTimeout(resolve, 2000)); // Validator sometimes needs time to start
    });

    before(async () => {
        // Airdrop SOL to test accounts
        const airdropPromises = [feeRecipientKey, ...contributorKeys].map(async (kp) => {
            return provider.connection.requestAirdrop(kp.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
        });
        await Promise.all(airdropPromises);

        // Calculate PDAs
        [registryPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("registry"), memeIdBuffer],
            program.programId
        );
        [vaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("vault"), memeIdBuffer],
            program.programId
        );
        [statePDA] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("state")],
            program.programId
        );
    });

    it("Initialize meme fund", async () => {
        const initialMinBuyAmount = new BN(100_000_000); // 0.1 SOL
        const initialMaxBuyAmount = new BN(1_000_000_000); // 1 SOL
        const initialFundDuration = new BN(300); // 5 minutes
        const initialMaxFundLimit = new BN(10_000_000_000); // 10 SOL
        const initialCommissionRate = 5; // 5%
        const initialTokenClaimAvailableTime = new BN(150); // 15 minutes (900) (for testing 2.5 minutes)

        await program.methods.initialize(
            feeRecipientKey.publicKey,
            initialMinBuyAmount,
            initialMaxBuyAmount,
            initialFundDuration,
            initialMaxFundLimit,
            initialCommissionRate,
            initialTokenClaimAvailableTime,
        ).accounts({
            authority: provider.wallet.publicKey,
        }).rpc();

        const state = await program.account.state.fetch(statePDA);
        assert.equal(state.feeRecipient.toBase58(), feeRecipientKey.publicKey.toBase58());
    });

    it("Creates meme registry", async () => {
        await program.methods.createMemeRegistry(memeId)
            .accounts({
                registry: registryPda,
                vault: vaultPda,
                state: statePDA,
                authority: provider.wallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        const registry = await program.account.memeRegistry.fetch(registryPda);
        assert.deepEqual(registry.memeId, memeId);
    });

    it("Allows contributions from multiple users", async () => {
        const amount = new BN(100_000_000); // 0.1 SOL

        for (const contributor of contributorKeys) {
            const [contributionPda] = PublicKey.findProgramAddressSync(
                [Buffer.from("contribution"), memeIdBuffer, contributor.publicKey.toBuffer()],
                program.programId
            );

            await program.methods.contribute(memeId, amount)
                .accounts({
                    contributor: contributor.publicKey,
                    feeRecipient: feeRecipientKey.publicKey,
                })
                .signers([contributor])
                .rpc();
        }

        const registry = await program.account.memeRegistry.fetch(registryPda);
        assert.equal(registry.contributorCount.toString(), contributorKeys.length.toString());
    });

    it("Starts meme creation", async () => {
      
        const [mintAuthority] = PublicKey.findProgramAddressSync(
            [Buffer.from('mint-authority')],
            PUMP_PROGRAM_ID
        );
        const [bondingCurve] = PublicKey.findProgramAddressSync(
            [Buffer.from('bonding-curve'), mint.publicKey.toBuffer()],
            PUMP_PROGRAM_ID
        );

        const [global] = PublicKey.findProgramAddressSync(
            [Buffer.from('global')],
            PUMP_PROGRAM_ID
        );
        
        const [metadata] = PublicKey.findProgramAddressSync(
          [Buffer.from('metadata'), MPL_TOKEN_METADATA.toBuffer(), mint.publicKey.toBuffer()],
          MPL_TOKEN_METADATA
        );

        const associatedBondingCurve = await getAssociatedTokenAddress(
            mint.publicKey,
            bondingCurve,
            true
        );

        const associatedUser = await getAssociatedTokenAddress(
            mint.publicKey,
            vaultPda,
            true
        );

      // Get global account data
      const globalAccountInfo = await provider.connection.getAccountInfo(global);
      const globalAccount = GlobalAccount.fromBuffer(globalAccountInfo.data);

      // Calculate buy amount
      const vaultAccountInfo = await provider.connection.getAccountInfo(vaultPda);
      const lamports = vaultAccountInfo.lamports;
      const lamportsBN = new BN(lamports).sub(new BN(30000000));
      const buyAmountSol = lamportsBN.mul(new BN(1e9)).div(new BN(anchor.web3.LAMPORTS_PER_SOL));
      const buyAmount = globalAccount.getInitialBuyPrice(buyAmountSol);
      const buyAmountWithSlippage = buyAmount.add(buyAmount.mul(SLIPPAGE_BASIS_POINTS).div(new BN(10000)));

      const modifyComputeBudgetIx = anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({
        units: 500000
      });

        await program.methods.startMeme(
            memeId,
            name,
            symbol,
            uri,
            buyAmount,
            buyAmountWithSlippage
        )
        .accounts({
            registry: registryPda,
            mint: mint.publicKey,
            mintAuthority,
            bondingCurve,
            associatedBondingCurve,
            global,
            mplTokenMetadata: MPL_TOKEN_METADATA,
            metadata,
            authority: provider.wallet.publicKey,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            eventAuthority: EVENT_AUTHORITY,
            pumpProgram: PUMP_PROGRAM_ID,
            feeRecipient: PUMP_FEE_RECIPIENT,
            associatedUser,
        })
        .preInstructions([modifyComputeBudgetIx])
        .signers([mint])
        .rpc();

        const registry = await program.account.memeRegistry.fetch(registryPda);
        assert.equal(registry.mint.toBase58(), mint.publicKey.toBase58());
    });

    // Helper function to get vault token account
    async function getVaultTokenAccount(vault: PublicKey): Promise<PublicKey> {
        const tokenAccounts = await provider.connection.getTokenAccountsByOwner(vault, { programId: TOKEN_PROGRAM_ID });
        if (tokenAccounts.value.length === 0) throw new Error("Vault token account not found");
        return tokenAccounts.value[0].pubkey;
    }

    it("Allows contributors to claim tokens", async () => {
      const newMint = mint.publicKey;
      await new Promise(resolve => setTimeout(resolve, 450000)); // 7.5 minutes
   
      for (const contributor of contributorKeys) {
          const [contributionPda] = PublicKey.findProgramAddressSync(
              [Buffer.from("contribution"), memeIdBuffer, contributor.publicKey.toBuffer()],
              program.programId
          );
   
          const userTokenAccount = await getAssociatedTokenAddress(
              newMint,
              contributor.publicKey
          );
   
          const vaultTokenAccount = await getVaultTokenAccount(vaultPda);
   
          const transaction = new Transaction().add(
              new TransactionInstruction({
                  keys: [
                      {pubkey: registryPda, isSigner: false, isWritable: true},
                      {pubkey: contributionPda, isSigner: false, isWritable: true}, 
                      {pubkey: contributor.publicKey, isSigner: true, isWritable: false},
                      {pubkey: vaultPda, isSigner: false, isWritable: false},
                      {pubkey: vaultTokenAccount, isSigner: false, isWritable: true},
                      {pubkey: userTokenAccount, isSigner: false, isWritable: true},
                      {pubkey: newMint, isSigner: false, isWritable: false},
                      {pubkey: statePDA, isSigner: false, isWritable: false},
                      {pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false},
                      {pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false},
                      {pubkey: SystemProgram.programId, isSigner: false, isWritable: false},
                  ],
                  programId: program.programId,
                  data: program.coder.instruction.encode("claimTokens", {memeId})
              })
          );
   
          const txId = await sendAndConfirmTransaction(
              provider.connection,
              transaction,
              [contributor]
          );
          console.log("Transaction ID:", txId);
      }
   });
});