export interface ClipItem {
  id: number;
  kind: "text" | "image" | "html";
  content: string | null;
  image_path: string | null;
  pinned: boolean;
  category: string | null;
  created_at: number;
}
