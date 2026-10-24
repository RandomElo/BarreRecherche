import { describe, expect, it } from 'vitest';
import { libelleVersion, versionApplication } from './version';

describe('version', () => {
  it('lit la version injectée au build', () => {
    expect(versionApplication).toBe('0.0.0-test');
  });

  it('formate le libellé affiché', () => {
    expect(libelleVersion('0.1.0')).toBe('Version 0.1.0');
  });
});
