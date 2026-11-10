import { pathModule } from "../node-shims.ts";

export default pathModule;
export const join = pathModule.join as (...parts: string[]) => string;
export const resolve = pathModule.resolve as (...parts: string[]) => string;
export const dirname = pathModule.dirname as (value: string) => string;
export const basename = pathModule.basename as (value: string, suffix?: string) => string;
export const extname = pathModule.extname as (value: string) => string;
export const normalize = pathModule.normalize as (value: string) => string;
export const relative = pathModule.relative as (from: string, to: string) => string;
export const isAbsolute = pathModule.isAbsolute as (value: string) => boolean;
export const sep = pathModule.sep as string;
export const delimiter = pathModule.delimiter as string;
