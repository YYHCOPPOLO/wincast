import { cryptoModule } from "../node-shims.ts";

export default cryptoModule;
export const { createHash, createHmac, randomBytes, randomUUID, randomInt, randomFillSync, webcrypto } = cryptoModule;
