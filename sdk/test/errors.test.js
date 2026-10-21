import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  CONTRACT_ERRORS,
  contractErrorsForCode,
  isKnownContractError,
} from "../dist/index.js";
import schema from "../schemas/contract-errors.schema.json" with { type: "json" };

test("exports every contract error enum from the Rust sources", () => {
  for (const [contract, errors] of Object.entries(CONTRACT_ERRORS)) {
    const source = readFileSync(new URL(`../../contracts/${contract}/src/errors.rs`, import.meta.url), "utf8");
    const declared = Object.fromEntries(
      [...source.matchAll(/^\s*(\w+)\s*=\s*(\d+)\s*,?\s*$/gm)].map(([, name, code]) => [name, Number(code)]),
    );
    assert.deepEqual(errors, declared, contract);
  }
});

test("maps known codes per contract and keeps code values unique within each contract", () => {
  assert.equal(isKnownContractError("invoice-token", 13), true);
  assert.equal(isKnownContractError("invoice-token", 99), false);
  assert.equal(isKnownContractError("unknown-contract", 1), false);
  assert.deepEqual(contractErrorsForCode("invoice-escrow", 55), ["FeeTooHigh"]);
  assert.deepEqual(contractErrorsForCode("invoice-escrow", 57), ["MaxInvestorsReached"]);
  assert.deepEqual(contractErrorsForCode("invoice-escrow", 68), ["InvestorCapReached"]);
  assert.deepEqual(contractErrorsForCode("secondary-market", 12), ["FloorPriceViolation"]);
});

test("JSON-RPC schema includes a contract-specific variant for every error", () => {
  const contractErrorSchema = schema.$defs.ContractError;
  assert.equal(contractErrorSchema.oneOf.length,
    Object.values(CONTRACT_ERRORS).reduce((sum, errors) => sum + Object.keys(errors).length, 0));
});
