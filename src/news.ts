import { createApp } from "vue";
import { createPinia } from "pinia";
import NewsApp from "./NewsApp.vue";
import "./styles.css";

createApp(NewsApp).use(createPinia()).mount("#app");
