import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { Vesting } from "../target/types/vesting";
import {
  createAssociatedTokenAccount,
  createMint,
  mintTo,
} from "@solana/spl-token";
import { assert, expect } from "chai";
import { SYSVAR_CLOCK_PUBKEY, PublicKey } from "@solana/web3.js";

const passphrase = "5";
const releaseOne = 100;
const releaseOneTime = 1658813160;
const releaseTwo = 120;
const releaseTwoTime = 1658813400;
const releaseThree = 130;
const releaseThreeTime = 1658814000;

describe("vesting", () => {
  const { web3 } = anchor;
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Vesting as Program<Vesting>;

  let mintAddress: PublicKey | null;
  let ownerToken: PublicKey | null;
  let employeeToken: PublicKey | null;

  const owner = web3.Keypair.generate();
  const employee = web3.Keypair.generate();

  it("Create vesting contract", async () => {
    console.log("==================== Creating Contract ====================");

    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(owner.publicKey, 1e9)
    );

    mintAddress = await createMint(
      provider.connection,
      owner,
      owner.publicKey,
      owner.publicKey,
      9
    );
    console.log(`Creating Mint: ${mintAddress}`);

    ownerToken = await createAssociatedTokenAccount(
      provider.connection,
      owner,
      mintAddress,
      owner.publicKey
    );

    await mintTo(
      provider.connection,
      owner,
      mintAddress,
      ownerToken,
      owner.publicKey,
      1e9
    );

    employeeToken = await createAssociatedTokenAccount(
      provider.connection,
      owner,
      mintAddress,
      employee.publicKey
    );

    const [vestingAccount, _vestingBump] =
      await web3.PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode(passphrase))],
        program.programId
      );

    console.log(`Vesting Account: ${vestingAccount}`);
    const [vestingTokenAccount, _vestingTokenBump] =
      await web3.PublicKey.findProgramAddress(
        [mintAddress.toBuffer(), vestingAccount.toBuffer()],
        program.programId
      );
    console.log(`Vesting Token Account: ${vestingTokenAccount}`);

    const tx = await program.methods
      .create(
        [
          new anchor.BN(releaseOneTime),
          new anchor.BN(releaseTwoTime),
          new anchor.BN(releaseThreeTime),
        ],
        [
          new anchor.BN(releaseOne),
          new anchor.BN(releaseTwo),
          new anchor.BN(releaseThree),
        ],
        passphrase
      )
      .accounts({
        initializer: owner.publicKey,
        vestingAccount,
        srcTokenAccount: ownerToken,
        dstTokenAccountOwner: employee.publicKey,
        dstTokenAccount: employeeToken,
        vestingTokenAccount,
        mintAddress,
      })
      .signers([owner])
      .rpc();
    console.log(`Transaction: ${tx}`);

    const escrowInfo = await program.account.vestingScheduleHeader.fetch(
      vestingAccount
    );
    const vestingAccountInfo: any =
      await provider.connection.getParsedAccountInfo(vestingTokenAccount);
    assert.equal(+escrowInfo.schedules[0].releaseTime, releaseOneTime);
    assert.equal(+escrowInfo.schedules[1].releaseTime, releaseTwoTime);
    assert.equal(+escrowInfo.schedules[2].releaseTime, releaseThreeTime);
    assert.equal(
      +vestingAccountInfo.value.data.parsed.info.tokenAmount.amount,
      releaseOne + releaseTwo + releaseThree
    );
  });

  //! =================================

  it("Trigger unlock", async () => {
    console.log("==================== Unlock ====================");
    const [vestingAccount, _vestingBump] =
      await web3.PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode(passphrase))],
        program.programId
      );

    const [vestingTokenAccount, _vestingTokenBump] =
      await web3.PublicKey.findProgramAddress(
        [mintAddress.toBuffer(), vestingAccount.toBuffer()],
        program.programId
      );

    const tx = await program.methods
      .unlock(passphrase)
      .accounts({
        vestingAccount,
        vestingTokenAccount,
        dstTokenAccount: employeeToken,
        mintAddress,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .rpc();

    console.log(`Transaction: ${tx}`);
    const employeeTokenAccountInfo: any =
      await provider.connection.getParsedAccountInfo(employeeToken);
    assert.equal(
      +employeeTokenAccountInfo.value.data.parsed.info.tokenAmount.amount,
      releaseOne + releaseTwo + releaseThree
    );
  });

  //! =================================

  it("Trigger unlock when zero, should fail", async () => {
    console.log(
      "==================== Unlock, Should Fail ===================="
    );
    const [vestingAccount, _vestingBump] =
      await web3.PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode(passphrase))],
        program.programId
      );

    const [vestingTokenAccount, _vestingTokenBump] =
      await web3.PublicKey.findProgramAddress(
        [mintAddress.toBuffer(), vestingAccount.toBuffer()],
        program.programId
      );

    try {
      const tx = await program.methods
        .unlock(passphrase)
        .accounts({
          vestingAccount,
          vestingTokenAccount,
          dstTokenAccount: employeeToken,
          mintAddress,
          clock: SYSVAR_CLOCK_PUBKEY,
        })
        .rpc();
    } catch (error) {
      console.log("Error Message: ", error.error.errorMessage);
      expect(true);
    }
  });

  //! =================================

  it("Change Destination", async () => {
    console.log("==================== Change Destination ====================");

    const [vestingAccount, _vestingBump] =
      await web3.PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode(passphrase))],
        program.programId
      );

    const newAddr = web3.Keypair.generate();
    const newAddrToken = await createAssociatedTokenAccount(
      provider.connection,
      owner,
      mintAddress,
      newAddr.publicKey
    );

    console.log(`New Address: ${newAddr.publicKey}`);

    const tx = await program.methods
      .changeDestination(passphrase)
      .accounts({
        vestingAccount,
        currentDestinationTokenAccount: employeeToken,
        currentDestinationTokenAccountOwner: employee.publicKey,
        newDestinationTokenAccount: newAddrToken,
        newDestinationTokenAccountOwner: newAddr.publicKey,
      })
      .signers([employee])
      .rpc();
    console.log(`Transaction: ${tx}`);

    const escrowInfo = await program.account.vestingScheduleHeader.fetch(
      vestingAccount
    );

    assert.equal(
      escrowInfo.destinationTokenAccountOwner.toString(),
      newAddr.publicKey.toString()
    );
  });

  //! =================================

  it("Close Vesting Contract", async () => {
    console.log(
      "==================== Closing Vesting Contract ===================="
    );

    const [vestingAccount, _vestingBump] =
      await web3.PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode(passphrase))],
        program.programId
      );

    const [vestingTokenAccount, _vestingTokenBump] =
      await web3.PublicKey.findProgramAddress(
        [mintAddress.toBuffer(), vestingAccount.toBuffer()],
        program.programId
      );

    const tx = await program.methods
      .closeAccount(passphrase)
      .accounts({
        vestingAccount,
        initializer: owner.publicKey,
        vestingTokenAccount,
        srcTokenAccount: ownerToken,
        mintAddress,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .signers([owner])
      .rpc();
    console.log(`Transaction: ${tx}`);
    try {
      await program.account.vestingScheduleHeader.fetch(vestingAccount);
      expect.fail();
    } catch (error) {
      expect(true);
    }
  });
});
