import { Router, Route } from "@solidjs/router";
import MainLayout from "./layouts/MainLayout";
import Home from "./pages/Home";
import Restaurants from "./pages/Restaurants";
import Orders from "./pages/Orders";
import Statistics from "./pages/Statistics";
import Admin from "./pages/Admin";
import Login from "./pages/Login";
import NotFound from "./pages/NotFound";

export default function App() {
  return (
    <Router root={MainLayout}>
      <Route path="/" component={Home} />
      <Route path="/restaurants" component={Restaurants} />
      <Route path="/orders" component={Orders} />
      <Route path="/statistics" component={Statistics} />
      <Route path="/admin" component={Admin} />
      <Route path="/login" component={Login} />
      <Route path="*404" component={NotFound} />
    </Router>
  );
}