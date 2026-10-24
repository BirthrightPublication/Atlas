import test from 'node:test';
import assert from 'node:assert/strict';
import { minorFromInput } from '../src/api.ts';

test('pasted negative amounts preserve sign and exact fractional units', () => {
  assert.equal(minorFromInput(' -0.50', 'USD'), -50);
  assert.equal(minorFromInput(' -1.50', 'USD'), -150);
  assert.equal(minorFromInput(' -0.50 ', 'USD'), -50);
  assert.equal(minorFromInput('\t-12.34\n', 'USD'), -1234);
  assert.equal(minorFromInput(' 19.99 ', 'USD'), 1999);
});

test('currency minor-unit precision is respected without float rounding', () => {
  assert.equal(minorFromInput('123', 'JPY'), 123);
  assert.equal(minorFromInput(' -0.125 ', 'KWD'), -125);
  assert.throws(() => minorFromInput('123.1', 'JPY'), /decimal places/);
  assert.throws(() => minorFromInput('0.001', 'USD'), /decimal places/);
});

test('unsafe or malformed money never reaches the service as a number', () => {
  assert.throws(() => minorFromInput('90071992547409.92', 'USD'), /too large/);
  for (const value of ['1e3', 'NaN', 'Infinity', '1,000.00', '']) {
    assert.throws(() => minorFromInput(value, 'USD'), /valid monetary amount/);
  }
});
