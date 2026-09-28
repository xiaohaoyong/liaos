// 数据模型类型定义（与 Rust 端结构体一一对应）

// 任务状态：待办 / 进行中 / 已完成 / 已取消
export type TaskStatus = "todo" | "doing" | "done" | "cancelled";

// 待办任务
export interface Task {
  id: string; // 唯一标识（UUID）
  title: string; // 标题 / 内容
  category: string; // 分类名
  status: TaskStatus;
  progress: number; // 进度 0-100（整数）
  actualStartTime?: string; // 开始时间（实际开始）
  plannedEndTime?: string; // 计划完成时间
  actualEndTime?: string; // 实际完成时间
  note?: string; // 备注
  createdAt: string; // 创建时间（ISO 8601）
  updatedAt: string; // 更新时间（ISO 8601）
}

// 分类
export interface Category {
  id: string;
  name: string;
  isPreset: boolean; // 是否预设（工作/生活/学习/其他为预设，不可删）
}

// 外观主题：跟随系统 / 亮色 / 暗色
export type ThemeMode = "system" | "light" | "dark";

// 应用配置
export interface Settings {
  remoteUrl: string; // 远程仓库 HTTPS 地址（Gitee/GitHub/自建均可）
  username: string; // HTTPS 认证用户名
  token: string; // Personal Access Token（作为密码）
  autoSync: boolean; // 是否自动同步
  autostart: boolean; // 是否开机自启
  theme: ThemeMode; // 外观主题
  hotkey: string; // 呼出/隐藏便签的全局快捷键
  hostsHotkey: string; // 呼出 Hosts 编辑器的全局快捷键
  newsFeeds: NewsFeed[]; // 新闻源列表（资讯窗口）
}

// 新闻源（RSS/Atom 订阅地址）
export interface NewsFeed {
  name: string; // 源显示名
  url: string; // RSS/Atom 地址
  enabled: boolean; // 是否启用
}

// 单条新闻（资讯窗口列表项）
export interface NewsItem {
  id: string; // 唯一标识（即链接）
  title: string; // 标题
  summary: string; // 内容简介（纯文本）
  link: string; // 原文链接
  source: string; // 来源名
  publishedAt: number; // 发布时间（unix 秒，0 表示未知）
}

// 新闻缓存（Rust 端 news_cache.json 对应结构）
export interface NewsCache {
  items: NewsItem[];
  fetchedAt: number | null; // 最近一次抓取时间（unix 秒）
}

// 一次性加载的完整应用数据
export interface AppData {
  tasks: Task[];
  categories: Category[];
  settings: Settings;
}

// 域名下的一个候选 IP（可带备注区分环境，如「开发」「生产」）
export interface HostsIp {
  id: string; // 唯一标识（UUID）
  ip: string; // IPv4/IPv6 地址
  note: string; // 备注（可空）
}

// 一个域名的完整配置：多个候选 IP，同一时刻仅 activeIpId 指向的 IP 生效
export interface HostsDomain {
  id: string; // 唯一标识（UUID）
  domain: string; // 域名，如 api.example.com
  enabled: boolean; // 域名开关：关闭则不写入 hosts
  ips: HostsIp[]; // 候选 IP 列表
  activeIpId: string | null; // 当前生效的候选 IP（null = 未选择）
}

// Hosts 编辑器完整配置（Rust 端 hosts.json 对应结构）
export interface HostsConfig {
  domains: HostsDomain[];
}

// 系统 hosts 标记块与当前配置是否一致
export interface HostsStatus {
  upToDate: boolean;
}

// 「应用到系统」的结果（UAC 取消走 cancelled 而非报错）
export interface ApplyOutcome {
  cancelled: boolean;
  message: string;
}

// ===== 便利贴（桌面常驻小纸片，无时间/状态/分类等任务属性） =====

// 便利贴纸片颜色（固定色板，不跟随深色模式）
export type NoteColor = "yellow" | "pink" | "blue" | "green" | "purple" | "gray";

// 一条便利贴（Rust 端 notes.json 对应结构）
export interface Note {
  id: string; // 唯一标识
  content: string; // 大段文字内容
  color: NoteColor; // 纸片颜色
  x: number; // 窗口左上角 X（逻辑坐标）
  y: number; // 窗口左上角 Y（逻辑坐标）
  width: number; // 窗口逻辑宽度（高度随内容自适应，不持久化）
  pinned: boolean; // 置顶（压在所有应用之上，且不受快捷键收起影响）
  createdAt: string; // 创建时间（ISO 8601）
  updatedAt: string; // 更新时间（ISO 8601，同步合并按此择新）
}

// 色板顺序（新建时按数量轮换）
export const NOTE_COLORS: NoteColor[] = ["yellow", "pink", "blue", "green", "purple", "gray"];

// 纸片配色：bg 纸面 / edge 底边描边（纸片厚度感）
export const NOTE_COLOR_MAP: Record<NoteColor, { bg: string; edge: string }> = {
  yellow: { bg: "#fcf3c5", edge: "#eddf9c" },
  pink: { bg: "#fadce6", edge: "#eec2d2" },
  blue: { bg: "#d8e9fa", edge: "#bcd4ef" },
  green: { bg: "#daedd2", edge: "#c2dcb4" },
  purple: { bg: "#e6dcf7", edge: "#d3c4ec" },
  gray: { bg: "#f3f2ee", edge: "#e0ded6" },
};

// 便利贴局部更新补丁（updateNote 只传要改的字段，Rust 端按 id 合并）
export type NotePatch = Partial<Pick<Note, "content" | "color" | "pinned" | "x" | "y" | "width">>;

// 状态展示映射
export const STATUS_LABEL: Record<TaskStatus, string> = {
  todo: "待办",
  doing: "进行中",
  done: "已完成",
  cancelled: "已取消",
};

// 「未完成」状态集合（默认展示）
export const UNFINISHED_STATUSES: TaskStatus[] = ["todo", "doing"];
