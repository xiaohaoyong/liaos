<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import {
  NModal,
  NForm,
  NFormItem,
  NInput,
  NSelect,
  NSlider,
  NDatePicker,
  NButton,
  NSpace,
  useMessage,
} from "naive-ui";
import type { Task, TaskStatus } from "../types";
import { STATUS_LABEL } from "../types";
import { useTasksStore } from "../stores/tasks";
import { useCategoriesStore } from "../stores/categories";
import { createEmptyTask, now } from "../utils";

const props = defineProps<{
  show: boolean;
  task: Task | null; // null 表示新建
}>();

const emit = defineEmits<{
  (e: "update:show", v: boolean): void;
}>();

const message = useMessage();
const tasksStore = useTasksStore();
const categoriesStore = useCategoriesStore();

const statusOptions = (Object.keys(STATUS_LABEL) as TaskStatus[]).map((k) => ({
  label: STATUS_LABEL[k],
  value: k,
}));

const categoryOptions = computed(() =>
  categoriesStore.categories.map((c) => ({ label: c.name, value: c.name }))
);

const form = reactive({
  title: "",
  category: "",
  status: "todo" as TaskStatus,
  progress: 0,
  actualStartTime: null as string | null,
  plannedEndTime: null as string | null,
  actualEndTime: null as string | null,
  note: "",
});

watch(
  () => props.show,
  (show) => {
    if (!show) return;
    if (props.task) {
      const t = props.task;
      form.title = t.title;
      form.category = t.category;
      form.status = t.status;
      form.progress = t.progress;
      form.actualStartTime = t.actualStartTime ?? null;
      form.plannedEndTime = t.plannedEndTime ?? null;
      form.actualEndTime = t.actualEndTime ?? null;
      form.note = t.note ?? "";
    } else {
      form.title = "";
      form.category = categoriesStore.categories[0]?.name ?? "";
      form.status = "todo";
      form.progress = 0;
      form.actualStartTime = null;
      form.plannedEndTime = null;
      form.actualEndTime = null;
      form.note = "";
    }
  }
);

function close() {
  emit("update:show", false);
}

function submit() {
  if (!form.title.trim()) {
    message.warning("请填写待办内容");
    return;
  }
  if (!form.category) {
    message.warning("请选择分类");
    return;
  }

  if (props.task) {
    // 编辑
    const t: Task = {
      ...props.task,
      title: form.title.trim(),
      category: form.category,
      status: form.status,
      progress: form.progress,
      actualStartTime: form.actualStartTime ?? undefined,
      plannedEndTime: form.plannedEndTime ?? undefined,
      actualEndTime: form.actualEndTime ?? undefined,
      note: form.note || undefined,
      updatedAt: now(),
    };
    tasksStore.update(t);
  } else {
    // 新建
    const t = createEmptyTask(form.category);
    t.title = form.title.trim();
    t.status = form.status;
    t.progress = form.progress;
    t.actualStartTime = form.actualStartTime ?? undefined;
    t.plannedEndTime = form.plannedEndTime ?? undefined;
    t.actualEndTime = form.actualEndTime ?? undefined;
    t.note = form.note || undefined;
    tasksStore.add(t);
  }
  close();
}
</script>

<template>
  <n-modal
    :show="show"
    :mask-closable="false"
    preset="card"
    :title="task ? '编辑待办' : '新建待办'"
    style="width: 560px; max-width: 90vw"
    @update:show="emit('update:show', $event)"
  >
    <n-form label-placement="top">
      <n-form-item label="内容" required>
        <n-input
          v-model:value="form.title"
          placeholder="要做什么？"
          autofocus
          @keyup.enter="submit"
        />
      </n-form-item>

      <n-form-item label="分类" required>
        <n-select v-model:value="form.category" :options="categoryOptions" />
      </n-form-item>

      <n-form-item label="状态">
        <n-select v-model:value="form.status" :options="statusOptions" />
      </n-form-item>

      <n-form-item label="进度">
        <div class="progress-row">
          <n-slider v-model:value="form.progress" :step="5" class="progress-slider" />
          <span class="progress-value">{{ form.progress }}%</span>
        </div>
      </n-form-item>

      <n-form-item label="开始时间（实际）">
        <n-date-picker
          v-model:formatted-value="form.actualStartTime"
          type="datetime"
          value-format="yyyy-MM-dd HH:mm:ss"
          clearable
          style="width: 100%"
        />
      </n-form-item>

      <n-form-item label="计划完成时间">
        <n-date-picker
          v-model:formatted-value="form.plannedEndTime"
          type="datetime"
          value-format="yyyy-MM-dd HH:mm:ss"
          clearable
          style="width: 100%"
        />
      </n-form-item>

      <n-form-item label="实际完成时间">
        <n-date-picker
          v-model:formatted-value="form.actualEndTime"
          type="datetime"
          value-format="yyyy-MM-dd HH:mm:ss"
          clearable
          style="width: 100%"
        />
      </n-form-item>

      <n-form-item label="备注">
        <n-input v-model:value="form.note" type="textarea" :rows="3" placeholder="补充说明（可选）" />
      </n-form-item>
    </n-form>

    <template #footer>
      <n-space justify="end">
        <n-button @click="close">取消</n-button>
        <n-button type="primary" @click="submit">{{ task ? "保存" : "添加" }}</n-button>
      </n-space>
    </template>
  </n-modal>
</template>

<style scoped>
.progress-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.progress-slider {
  flex: 1;
}
.progress-value {
  min-width: 40px;
  text-align: right;
  font-size: 13px;
  color: var(--text-2);
}
</style>
