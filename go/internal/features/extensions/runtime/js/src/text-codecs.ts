// Text codecs. Defined before any bundled dependency runs, because a few
// packages read TextEncoder at module evaluation time.

export class TextEncoder {
  readonly encoding = "utf-8";
  encode(input = ""): Uint8Array {
    const output: number[] = [];
    for (let index = 0; index < input.length; index++) {
      let code = input.charCodeAt(index);
      if (code >= 0xd800 && code <= 0xdbff && index + 1 < input.length) {
        const low = input.charCodeAt(index + 1);
        if (low >= 0xdc00 && low <= 0xdfff) {
          code = ((code - 0xd800) << 10) + (low - 0xdc00) + 0x10000;
          index++;
        }
      }
      if (code < 0x80) {
        output.push(code);
      } else if (code < 0x800) {
        output.push(0xc0 | (code >> 6), 0x80 | (code & 63));
      } else if (code < 0x10000) {
        output.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63));
      } else {
        output.push(
          0xf0 | (code >> 18),
          0x80 | ((code >> 12) & 63),
          0x80 | ((code >> 6) & 63),
          0x80 | (code & 63),
        );
      }
    }
    return Uint8Array.from(output);
  }
}

export class TextDecoder {
  readonly encoding: string;
  readonly fatal: boolean;
  readonly ignoreBOM: boolean;
  constructor(label = "utf-8", options: { fatal?: boolean; ignoreBOM?: boolean } = {}) {
    this.encoding = String(label).toLowerCase();
    this.fatal = !!options.fatal;
    this.ignoreBOM = !!options.ignoreBOM;
  }
  decode(input?: ArrayBuffer | ArrayBufferView, options: { stream?: boolean } = {}): string {
    if (!input) return "";
    const bytes =
      input instanceof Uint8Array
        ? input
        : ArrayBuffer.isView(input)
          ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
          : new Uint8Array(input);
    let output = "";
    for (let index = 0; index < bytes.length; ) {
      const first = bytes[index++];
      if (first < 0x80) {
        output += String.fromCharCode(first);
        continue;
      }
      let code = 0;
      let remaining = 0;
      if ((first & 0xe0) === 0xc0) {
        code = first & 0x1f;
        remaining = 1;
      } else if ((first & 0xf0) === 0xe0) {
        code = first & 0x0f;
        remaining = 2;
      } else if ((first & 0xf8) === 0xf0) {
        code = first & 0x07;
        remaining = 3;
      } else {
        if (this.fatal) throw new TypeError("The encoded data was not valid for encoding utf-8");
        output += "\ufffd";
        continue;
      }
      if (index + remaining > bytes.length) {
        if (this.fatal && !options.stream) {
          throw new TypeError("The encoded data was not valid for encoding utf-8");
        }
        output += "\ufffd";
        break;
      }
      for (let count = 0; count < remaining; count++) {
        const next = bytes[index++];
        if ((next & 0xc0) !== 0x80) {
          if (this.fatal) throw new TypeError("The encoded data was not valid for encoding utf-8");
          output += "\ufffd";
          break;
        }
        code = (code << 6) | (next & 63);
      }
      if (code >= 0x10000) {
        code -= 0x10000;
        output += String.fromCharCode(0xd800 + (code >> 10), 0xdc00 + (code & 0x3ff));
      } else {
        output += String.fromCharCode(code);
      }
    }
    if (!this.ignoreBOM && output.charCodeAt(0) === 0xfeff) output = output.slice(1);
    return output;
  }
}
