export const VERSION_REGEX = /^\d{1,2}\.\d{1,2}\.\d{1,3}$/

export function validateVersion(value: string, label: string): string | null {
  if (!VERSION_REGEX.test(value)) {
    return `${label} must be in x.y.z format (e.g., 1.2.34)`
  }
  try {
    parseVersion(value)
  } catch {
    return `${label} has invalid version components`
  }
  return null
}

export function formatVersion(version: number): string {
  const major = Math.floor(version / 100000);
  const minor = Math.floor((version % 100000) / 1000);
  const patch = version % 1000;
  return `${major}.${minor}.${patch}`;
}

export function parseVersion(versionStr: string): number {
  const parts = versionStr.split('.').map(Number);
  if (parts.length !== 3 || parts.some(isNaN)) {
    throw new Error("Invalid version string format. Expected 'major.minor.patch'");
  }
  const [major, minor, patch] = parts;
  if (major > 99 || minor > 99 || patch > 999) {
    throw new Error("Version component out of valid range.");
  }
  return major * 100000 + minor * 1000 + patch;
}
