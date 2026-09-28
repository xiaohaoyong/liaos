import { createApp } from "vue";
import { createPinia } from "pinia";
import NoteApp from "./NoteApp.vue";
import "./styles.css";

createApp(NoteApp).use(createPinia()).mount("#app");
