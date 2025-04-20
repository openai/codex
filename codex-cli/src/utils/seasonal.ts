// Default spinner frames (bouncing ball)
export const defaultSpinnerFrames: string[] = [
  "( ●    )",
  "(  ●   )",
  "(   ●  )",
  "(    ● )",
  "(     ●)",
  "(    ● )",
  "(   ●  )",
  "(  ●   )",
  "( ●    )",
  "(●     )",
];

/**
 * Returns a set of spinner frames, substituting the ball character
 * with a seasonal emoji if within one week of certain holidays.
 */
export function getSeasonalFrames(
  defaultFrames: string[] = defaultSpinnerFrames,
): string[] {
  // Check for FAKE_DATE environment variable
  const now = process.env['FAKE_DATE']
    ? new Date(process.env['FAKE_DATE'])
    : new Date();
    
  const year = now.getFullYear();
  const msPerDay = 1000 * 60 * 60 * 24;

  function isWithinDays(date: Date, target: Date, days: number): boolean {
    const start = new Date(target.getTime() - days * msPerDay);
    const end = new Date(target.getTime() + days * msPerDay);
    return date >= start && date <= end;
  }

  // Halloween: October 31
  const halloween = new Date(year, 9, 31);
  if (isWithinDays(now, halloween, 7)) {
    return defaultFrames.map((f) => f.replace('●', '🎃'));
  }

  // Christmas: December 25 (check current and previous year)
  const christmasCurrent = new Date(year, 11, 25);
  const christmasPrev = new Date(year - 1, 11, 25);
  if (
    isWithinDays(now, christmasCurrent, 7) ||
    isWithinDays(now, christmasPrev, 7)
  ) {
    return defaultFrames.map((f) => f.replace('●', '🎄'));
  }

  // Easter: calculate date -- Yes this is extreme... but its fun. - Crazywolf132
  function calcEaster(y: number): Date {
    const a = y % 19;
    const b = Math.floor(y / 100);
    const c = y % 100;
    const d = Math.floor(b / 4);
    const e = b % 4;
    const f = Math.floor((b + 8) / 25);
    const g = Math.floor((b - f + 1) / 3);
    const h = (19 * a + b - d - g + 15) % 30;
    const i = Math.floor(c / 4);
    const k = c % 4;
    const L = (32 + 2 * e + 2 * i - h - k) % 7;
    const m = Math.floor((a + 11 * h + 22 * L) / 451);
    const month = Math.floor((h + L - 7 * m + 114) / 31);
    const day = ((h + L - 7 * m + 114) % 31) + 1;
    return new Date(y, month - 1, day);
  }
  const easter = calcEaster(year);
  if (isWithinDays(now, easter, 7)) {
    // Easter: use rabbit emoji
    return defaultFrames.map((f) => f.replace('●', '🐰'));
  }

  return defaultFrames;
}