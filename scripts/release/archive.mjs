import path from 'node:path';

const ZIP_DATE = new Date('2026-01-01T00:00:00Z');
const CRC32_TABLE = new Uint32Array(256);
for (let index = 0; index < CRC32_TABLE.length; index += 1) {
  let value = index;
  for (let bit = 0; bit < 8; bit += 1) {
    value = (value & 1) !== 0 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  }
  CRC32_TABLE[index] = value >>> 0;
}

export function createZip(entries) {
  const records = [];
  const chunks = [];
  let offset = 0;
  for (const entry of entries) {
    const relativePath = normalizeArchivePath(entry.relativePath);
    const name = Buffer.from(relativePath, 'utf8');
    const data = Buffer.from(entry.data);
    const crc = crc32(data);
    const header = Buffer.alloc(30);
    header.writeUInt32LE(0x04034b50, 0);
    header.writeUInt16LE(20, 4);
    header.writeUInt16LE(0x0800, 6);
    writeDosDateTime(header, 10, ZIP_DATE);
    header.writeUInt32LE(crc, 14);
    header.writeUInt32LE(data.length, 18);
    header.writeUInt32LE(data.length, 22);
    header.writeUInt16LE(name.length, 26);
    chunks.push(header, name, data);
    records.push({ name, crc, mode: entry.mode ?? 0o644, offset, size: data.length });
    offset += header.length + name.length + data.length;
  }
  const directoryOffset = offset;
  for (const record of records) {
    const header = Buffer.alloc(46);
    header.writeUInt32LE(0x02014b50, 0);
    header.writeUInt16LE(20, 4);
    header.writeUInt16LE(20, 6);
    header.writeUInt16LE(0x0800, 8);
    writeDosDateTime(header, 12, ZIP_DATE);
    header.writeUInt32LE(record.crc, 16);
    header.writeUInt32LE(record.size, 20);
    header.writeUInt32LE(record.size, 24);
    header.writeUInt16LE(record.name.length, 28);
    header.writeUInt32LE((record.mode & 0xffff) << 16, 38);
    header.writeUInt32LE(record.offset, 42);
    chunks.push(header, record.name);
    offset += header.length + record.name.length;
  }
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(records.length, 8);
  end.writeUInt16LE(records.length, 10);
  end.writeUInt32LE(offset - directoryOffset, 12);
  end.writeUInt32LE(directoryOffset, 16);
  chunks.push(end);
  return Buffer.concat(chunks);
}

export function createTar(entries) {
  const chunks = [];
  for (const entry of entries) {
    const data = Buffer.from(entry.data);
    const name = normalizeArchivePath(entry.relativePath);
    chunks.push(createTarHeader(name, data.length, entry.mode ?? 0o644), data);
    chunks.push(Buffer.alloc((512 - (data.length % 512)) % 512));
  }
  chunks.push(Buffer.alloc(1024));
  return Buffer.concat(chunks);
}

function createTarHeader(name, size, mode) {
  const tarPath = splitTarPath(name);
  const header = Buffer.alloc(512);
  Buffer.from(tarPath.name, 'utf8').copy(header, 0);
  Buffer.from(tarPath.prefix, 'utf8').copy(header, 345);
  writeTarOctal(header, 100, 8, mode);
  writeTarOctal(header, 108, 8, 0);
  writeTarOctal(header, 116, 8, 0);
  writeTarOctal(header, 124, 12, size);
  writeTarOctal(header, 136, 12, 0);
  header.fill(0x20, 148, 156);
  header[156] = 0x30;
  Buffer.from('ustar\0', 'ascii').copy(header, 257);
  Buffer.from('00', 'ascii').copy(header, 263);
  writeTarChecksum(header, header.reduce((sum, byte) => sum + byte, 0));
  return header;
}

function splitTarPath(name) {
  if (Buffer.byteLength(name) <= 100) return { name, prefix: '' };
  const parts = name.split('/');
  for (let index = parts.length - 1; index > 0; index -= 1) {
    const prefix = parts.slice(0, index).join('/');
    const basename = parts.slice(index).join('/');
    if (Buffer.byteLength(prefix) <= 155 && Buffer.byteLength(basename) <= 100) {
      return { name: basename, prefix };
    }
  }
  throw new Error(`tar entry path is too long: ${name}`);
}

function normalizeArchivePath(value) {
  const normalized = String(value).replaceAll('\\', '/').replace(/^\/+|\/+$/gu, '');
  if (!normalized || normalized.split('/').includes('..') || path.isAbsolute(normalized)) {
    throw new Error(`unsafe archive path: ${value}`);
  }
  return normalized;
}

function writeTarOctal(buffer, offset, length, value) {
  buffer.write(value.toString(8).padStart(length - 1, '0').slice(-(length - 1)), offset, length - 1, 'ascii');
}

function writeTarChecksum(buffer, value) {
  buffer.write(value.toString(8).padStart(6, '0').slice(-6), 148, 6, 'ascii');
  buffer[155] = 0x20;
}

function writeDosDateTime(buffer, offset, date) {
  const time = (date.getUTCHours() << 11) | (date.getUTCMinutes() << 5) | Math.floor(date.getUTCSeconds() / 2);
  const day = ((date.getUTCFullYear() - 1980) << 9) | ((date.getUTCMonth() + 1) << 5) | date.getUTCDate();
  buffer.writeUInt16LE(time, offset);
  buffer.writeUInt16LE(day, offset + 2);
}

function crc32(buffer) {
  let value = 0xffffffff;
  for (const byte of buffer) value = CRC32_TABLE[(value ^ byte) & 0xff] ^ (value >>> 8);
  return (value ^ 0xffffffff) >>> 0;
}
