import { Router, Route } from "@solidjs/router";
import { onMount, Switch, Match } from "solid-js";
import MainLayout from "./layouts/MainLayout";
import Home from "./pages/Home";
import Restaurants from "./pages/Restaurants";
import RestaurantPage from "./pages/RestaurantPage";
import MenuEditor from "./pages/MenuEditor";
import Orders from "./pages/Orders";
import Statistics from "./pages/Statistics";
import Admin from "./pages/Admin";
import Login from "./pages/Login";
import NotFound from "./pages/NotFound";
import Setup from "./pages/Setup";
import { setupCompleted, setupLoading, setupError, checkSetupStatus } from "./stores/setupStore";
import { checkAuth } from "./stores/authStore";

function SetupLayout(props: { children?: any }) {
  return <>{props.children}</>;
}

export default function App() {
  onMount(() => {
    checkSetupStatus();
    checkAuth();
  });

  return (
    <Switch fallback={
      <div class="hero is-fullheight">
        <div class="hero-body is-justify-content-center">
          <div class="has-text-centered">
            <p class="title">🐔</p>
            <progress class="progress is-primary is-small" max="100" />
          </div>
        </div>
      </div>
    }>
      <Match when={!setupLoading() && setupCompleted() === null && setupError()}>
        <div class="hero is-fullheight">
          <div class="hero-body is-justify-content-center">
            <div class="has-text-centered">
              <p class="title">🐔</p>
              <p class="subtitle has-text-danger">Unable to reach the server</p>
              <p class="mb-4">{setupError()}</p>
              <button class="button is-primary" onClick={() => checkSetupStatus()}>
                Retry
              </button>
            </div>
          </div>
        </div>
      </Match>
      <Match when={!setupLoading() && setupCompleted() === false}>
        <Router root={SetupLayout}>
          <Route path="*" component={Setup} />
        </Router>
      </Match>
      <Match when={!setupLoading() && setupCompleted() === true}>
        <Router root={MainLayout}>
          <Route path="/" component={Home} />
          <Route path="/restaurants" component={Restaurants} />
          <Route path="/restaurants/:id" component={RestaurantPage} />
          <Route path="/restaurants/:id/menus/new" component={MenuEditor} />
          <Route path="/restaurants/:id/menus/:menuId/edit" component={MenuEditor} />
          <Route path="/orders" component={Orders} />
          <Route path="/statistics" component={Statistics} />
          <Route path="/admin" component={Admin} />
          <Route path="/login" component={Login} />
          <Route path="*404" component={NotFound} />
        </Router>
      </Match>
    </Switch>
  );
}