<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  NModal,
  NForm,
  NFormItem,
  NInput,
  NSelect,
  NButton,
  NSpace,
  useMessage,
} from "naive-ui";
import { useTasksStore } from "../stores/tasks";
import { useCategoriesStore } from "../stores/categories";
import { createEmptyTask, now } from "../utils";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const message = useMessage();
const tasksStore = useTasksStore();
const categoriesStore = useCategoriesStore();

const title = ref("");
const category = ref("");

const categoryOptions = computed(() =>
  categoriesStore.categories.map((c) => ({ label: c.name, value: c.name }))
);

watch(
  () => props.show,
  (show) => {
    if (show) {
      title.value = "";
      category.value = categoriesStore.categories[0]?.name ?? "";
    }
  }
);

function close() {
  emit("update:show", false);
}

function submit() {
  const t = title.value.trim();
  if (!t) {
    message.warning("请输入待办内容");
    return;
  }
  if (!category.value) {
    message.warning("请选择分类");
    return;
  }
  const task = createEmptyTask(category.value);
  task.title = t;
  task.actualStartTime = now(); // 快捷添加时把添加时间记为开始时间
  tasksStore.add(task);
  close();
}
</script>

<template>
  <n-modal
    :show="show"
    :mask-closable="false"
    preset="card"
    title="快速添加"
    style="width: 280px"
    @update:show="emit('update:show', $event)"
  >
    <n-form label-placement="top">
      <n-form-item label="内容" required>
        <n-input
          v-model:value="title"
          placeholder="要做什么？"
          autofocus
          @keyup.enter="submit"
        />
      </n-form-item>
      <n-form-item label="分类" required>
        <n-select v-model:value="category" :options="categoryOptions" />
      </n-form-item>
    </n-form>
    <template #footer>
      <n-space justify="end">
        <n-button @click="close">取消</n-button>
        <n-button type="primary" @click="submit">添加</n-button>
      </n-space>
    </template>
  </n-modal>
</template>
