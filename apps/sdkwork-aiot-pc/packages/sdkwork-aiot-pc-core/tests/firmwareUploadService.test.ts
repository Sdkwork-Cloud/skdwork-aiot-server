import { describe, expect, it } from 'vitest';

import { sha256HexFromFile } from '../src/services/firmwareUploadService';

describe('firmwareUploadService', () => {
  it('computes sha256 hex digests for firmware files', async () => {
    const file = new File([new Uint8Array([0x61, 0x62, 0x63])], 'firmware.bin', {
      type: 'application/octet-stream',
    });

    await expect(sha256HexFromFile(file)).resolves.toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    );
  });
});
