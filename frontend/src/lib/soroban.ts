/**
 * Soroban contract interaction helpers.
 * Builds, simulates, and submits transactions.
 */
import {
  SorobanRpc,
  TransactionBuilder,
  Networks,
  BASE_FEE,
  Contract,
  nativeToScVal,
  Address,
  xdr,
} from "@stellar/stellar-sdk";
import { signTransaction } from "./freighter";

const RPC_URL = process.env.NEXT_PUBLIC_SOROBAN_RPC_URL!;
const NETWORK_PASSPHRASE = process.env.NEXT_PUBLIC_NETWORK_PASSPHRASE!;
const REWARDS_CONTRACT_ID = process.env.NEXT_PUBLIC_REWARDS_CONTRACT_ID!;
const CAMPAIGN_CONTRACT_ID = process.env.NEXT_PUBLIC_CAMPAIGN_CONTRACT_ID!;

const server = new SorobanRpc.Server(RPC_URL, { allowHttp: true });

async function invokeContract(
  publicKey: string,
  contractId: string,
  method: string,
  args: xdr.ScVal[]
): Promise<SorobanRpc.Api.GetSuccessfulTransactionResponse> {
  const account = await server.getAccount(publicKey);
  const contract = new Contract(contractId);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(contract.call(method, ...args))
    .setTimeout(30)
    .build();

  const simResult = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(simResult)) {
    throw new Error(`Simulation failed: ${simResult.error}`);
  }

  const preparedTx = SorobanRpc.assembleTransaction(tx, simResult).build();
  const signedXdr = await signTransaction(preparedTx.toXDR(), NETWORK_PASSPHRASE);

  const submitResult = await server.sendTransaction(
    TransactionBuilder.fromXDR(signedXdr, NETWORK_PASSPHRASE)
  );

  if (submitResult.status === "ERROR") {
    throw new Error(`Submit failed: ${submitResult.errorResult?.toXDR("base64")}`);
  }

  // Poll for confirmation
  let getResult = await server.getTransaction(submitResult.hash);
  let attempts = 0;
  while (
    getResult.status === SorobanRpc.Api.GetTransactionStatus.NOT_FOUND &&
    attempts < 20
  ) {
    await new Promise((r) => setTimeout(r, 1500));
    getResult = await server.getTransaction(submitResult.hash);
    attempts++;
  }

  if (getResult.status !== SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`Transaction failed: ${getResult.status}`);
  }

  return getResult as SorobanRpc.Api.GetSuccessfulTransactionResponse;
}

export async function claimReward(publicKey: string, campaignId: number) {
  return invokeContract(publicKey, REWARDS_CONTRACT_ID, "claim_reward", [
    new Address(publicKey).toScVal(),
    nativeToScVal(campaignId, { type: "u64" }),
  ]);
}

export async function redeemReward(publicKey: string, amount: bigint) {
  return invokeContract(publicKey, REWARDS_CONTRACT_ID, "redeem_reward", [
    new Address(publicKey).toScVal(),
    nativeToScVal(amount, { type: "i128" }),
  ]);
}

export async function createCampaign(
  publicKey: string,
  rewardAmount: bigint,
  expiration: number
) {
  return invokeContract(publicKey, CAMPAIGN_CONTRACT_ID, "create_campaign", [
    new Address(publicKey).toScVal(),
    nativeToScVal(rewardAmount, { type: "i128" }),
    nativeToScVal(expiration, { type: "u64" }),
  ]);
}

export async function deactivateCampaign(campaignId: number, publicKey: string) {
  return invokeContract(publicKey, CAMPAIGN_CONTRACT_ID, "deactivate_campaign", [
    new Address(publicKey).toScVal(),
    nativeToScVal(campaignId, { type: "u64" }),
  ]);
}
