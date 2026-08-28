import { createApp } from "vue";
import { createPinia } from "pinia";
import HostsApp from "./HostsApp.vue";
import "./styles.css";

createApp(HostsApp).use(createPinia()).mount("#app");
