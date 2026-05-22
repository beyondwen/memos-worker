export interface PersonalModeFeatures {
  publicDiscovery: boolean;
  inbox: boolean;
  integrations: boolean;
  socialActions: boolean;
  publicSharing: boolean;
  rss: boolean;
  accessTokens: boolean;
  audit: boolean;
}

export const PERSONAL_MODE_FEATURES: PersonalModeFeatures = {
  publicDiscovery: false,
  inbox: false,
  integrations: false,
  socialActions: false,
  publicSharing: false,
  rss: false,
  accessTokens: false,
  audit: false,
};

export interface PrimaryNavItem {
  id: "home" | "explore" | "timeline" | "inbox" | "settings";
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
    if (features.inbox) {
      items.push({ id: "inbox", href: "/inbox", label: "通知" });
    }
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
    if (tab.id === "integrations") return features.integrations;
    if (tab.id === "audit") return features.audit;
    return true;
  });
}
