export interface PersonalModeFeatures {
  publicDiscovery: boolean;
  rss: boolean;
  audit: boolean;
}

export const PERSONAL_MODE_FEATURES: PersonalModeFeatures = {
  publicDiscovery: false,
  rss: false,
  audit: false,
};

export interface PrimaryNavItem {
  id: "home" | "explore" | "timeline" | "settings";
  href: string;
  label: string;
}

export interface SettingsTabLike {
  id: string;
  adminOnly?: boolean;
}

export function personalPrimaryNavItems(
  authenticated: boolean,
  features: PersonalModeFeatures = PERSONAL_MODE_FEATURES,
): PrimaryNavItem[] {
  const items: PrimaryNavItem[] = [{ id: "home", href: "/", label: "首页" }];
  if (features.publicDiscovery) {
    items.push({ id: "explore", href: "/explore", label: "发现" });
  }
  if (authenticated) {
    items.push({ id: "timeline", href: "/timeline", label: "时间线" });
    items.push({ id: "settings", href: "/settings", label: "设置" });
  }
  return items;
}

export function personalSettingsTabs<T extends SettingsTabLike>(
  tabs: T[],
  role: string,
  features: PersonalModeFeatures = PERSONAL_MODE_FEATURES,
): T[] {
  return tabs.filter((tab) => {
    if (tab.adminOnly && role !== "ADMIN") return false;
    if (tab.id === "audit") return features.audit;
    return true;
  });
}
