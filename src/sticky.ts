import { createApp } from "vue";
import { createPinia } from "pinia";
import StickyApp from "./StickyApp.vue";
import "./styles.css";

createApp(StickyApp).use(createPinia()).mount("#app");
