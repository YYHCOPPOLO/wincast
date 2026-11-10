import { fsModule } from "../node-shims.ts";

export default fsModule;
export const {
  readFileSync,
  writeFileSync,
  appendFileSync,
  existsSync,
  statSync,
  lstatSync,
  readdirSync,
  mkdirSync,
  rmSync,
  rmdirSync,
  unlinkSync,
  renameSync,
  copyFileSync,
  realpathSync,
  accessSync,
  mkdtempSync,
  chmodSync,
  utimesSync,
  constants,
  promises,
} = fsModule;
