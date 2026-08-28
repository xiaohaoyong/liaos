import { defineStore } from "pinia";
import { ref } from "vue";
import type { Category } from "../types";
import { saveCategories } from "../api";

export const useCategoriesStore = defineStore("categories", () => {
  const categories = ref<Category[]>([]);

  function setCategories(list: Category[]) {
    categories.value = list;
  }

  function persist() {
    saveCategories(categories.value).catch((e) => console.error("保存分类失败", e));
  }

  function add(name: string) {
    const c: Category = {
      id: crypto.randomUUID(),
      name,
      isPreset: false,
    };
    categories.value.push(c);
    persist();
    return c;
  }

  function remove(id: string) {
    const c = categories.value.find((c) => c.id === id);
    if (c && c.isPreset) return; // 预设分类不可删除
    categories.value = categories.value.filter((c) => c.id !== id);
    persist();
  }

  function rename(id: string, name: string) {
    const c = categories.value.find((c) => c.id === id);
    if (c) {
      c.name = name;
      persist();
    }
  }

  return { categories, setCategories, add, remove, rename };
});
