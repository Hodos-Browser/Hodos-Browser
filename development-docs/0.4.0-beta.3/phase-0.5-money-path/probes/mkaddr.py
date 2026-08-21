import hashlib

ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'


def b58decode(s):
    n = 0
    for ch in s:
        n = n * 58 + ALPHABET.index(ch)
    full = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    pad = len(s) - len(s.lstrip('1'))
    return b'\x00' * pad + full


def b58encode(b):
    n = int.from_bytes(b, 'big')
    out = ''
    while n:
        n, r = divmod(n, 58)
        out = ALPHABET[r] + out
    pad = len(b) - len(b.lstrip(b'\x00'))
    return '1' * pad + out


def check(addr):
    raw = b58decode(addr)
    if len(raw) != 25:
        return 'BAD_LENGTH(%d)' % len(raw)
    payload, cs = raw[:21], raw[21:]
    want = hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    return 'VALID' if cs == want else 'BAD_CHECKSUM'


base = '1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2'
print('base       ', base, check(base))

# Flip one payload byte, keep the ORIGINAL checksum -> valid prefix, valid
# length, valid base58, invalid checksum.
raw = bytearray(b58decode(base))
raw[10] ^= 0x01
bad = b58encode(bytes(raw))
print('probe      ', bad, check(bad), 'version_byte=0x%02x' % raw[0])
