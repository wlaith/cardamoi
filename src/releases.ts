import type { Found } from "./api";

export type Availability = "Good" | "Fair" | "Few" | "None" | "Unknown";

export function availability(seeders: number | null): Availability {
  if (seeders == null) return "Unknown";
  if (seeders >= 20) return "Good";
  if (seeders >= 5) return "Fair";
  if (seeders >= 1) return "Few";
  return "None";
}

export function qualityRank(resolution: string | null): number {
  switch (resolution) {
    case "1080p":
      return 4;
    case "2160p":
      return 3;
    case "720p":
      return 2;
    default:
      return 1;
  }
}

export function rankReleases(results: Found[]): Found[] {
  return [...results].sort((a, b) => {
    const dead = Number(a.seeders === 0) - Number(b.seeders === 0);
    if (dead) return dead;
    const healthy = Number((b.seeders ?? 0) >= 5) - Number((a.seeders ?? 0) >= 5);
    if (healthy) return healthy;
    return qualityRank(b.release.resolution) - qualityRank(a.release.resolution) || (b.seeders ?? 0) - (a.seeders ?? 0);
  });
}

export function bestRelease(ranked: Found[]): Found | null {
  const top = ranked[0];
  return top && (top.seeders ?? 1) > 0 ? top : null;
}
