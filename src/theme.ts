import type { GlobalThemeOverrides } from "naive-ui";

// ===== 设计 Token =====
// 与 styles.css 中的 CSS 变量保持一致（简约 + Teal 强调 + 亮暗双主题）

const fontFamily =
  '-apple-system, "Segoe UI", "Microsoft YaHei", "PingFang SC", sans-serif';

interface Palette {
  primary: string;
  primaryHover: string;
  primaryPressed: string;
  primarySuppl: string;
  info: string;
  success: string;
  warning: string;
  error: string;
  body: string;
  card: string;
  text1: string;
  text2: string;
  text3: string;
  border: string;
}

const light: Palette = {
  primary: "#0D9488",
  primaryHover: "#0F766E",
  primaryPressed: "#115E59",
  primarySuppl: "#14B8A6",
  info: "#3B82F6",
  success: "#10B981",
  warning: "#F59E0B",
  error: "#DC2626",
  body: "#F8F9FA",
  card: "#FFFFFF",
  text1: "#18181B",
  text2: "#3F3F46",
  text3: "#6B7280",
  border: "#E5E7EB",
};

const dark: Palette = {
  primary: "#14B8A6",
  primaryHover: "#2DD4BF",
  primaryPressed: "#0D9488",
  primarySuppl: "#0D9488",
  info: "#60A5FA",
  success: "#34D399",
  warning: "#FBBF24",
  error: "#F87171",
  body: "#0F172A",
  card: "#1E293B",
  text1: "#F1F5F9",
  text2: "#CBD5E1",
  text3: "#94A3B8",
  border: "#334155",
};

function buildOverrides(p: Palette): GlobalThemeOverrides {
  return {
    common: {
      primaryColor: p.primary,
      primaryColorHover: p.primaryHover,
      primaryColorPressed: p.primaryPressed,
      primaryColorSuppl: p.primarySuppl,
      infoColor: p.info,
      infoColorHover: p.info,
      infoColorPressed: p.info,
      infoColorSuppl: p.info,
      successColor: p.success,
      successColorHover: p.success,
      successColorPressed: p.success,
      successColorSuppl: p.success,
      warningColor: p.warning,
      warningColorHover: p.warning,
      warningColorPressed: p.warning,
      warningColorSuppl: p.warning,
      errorColor: p.error,
      errorColorHover: p.error,
      errorColorPressed: p.error,
      errorColorSuppl: p.error,
      textColorBase: p.text1,
      textColor1: p.text1,
      textColor2: p.text2,
      textColor3: p.text3,
      borderColor: p.border,
      dividerColor: p.border,
      bodyColor: p.body,
      cardColor: p.card,
      modalColor: p.card,
      popoverColor: p.card,
      borderRadius: "6px",
      borderRadiusSmall: "4px",
      fontFamily,
    },
  };
}

export const lightThemeOverrides = buildOverrides(light);
export const darkThemeOverrides = buildOverrides(dark);
