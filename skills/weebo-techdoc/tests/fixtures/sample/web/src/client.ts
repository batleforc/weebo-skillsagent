const API = import.meta.env.VITE_API_URL ?? "http://localhost:3000";

export interface Item {
  id: number;
  name: string;
}

export async function listItems(): Promise<Item[]> {
  const res = await fetch(`${API}/items`);
  return res.json();
}

export async function createItem(name: string): Promise<Item> {
  const res = await fetch(`${API}/items`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
  return res.json();
}
