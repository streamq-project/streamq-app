declare module "*.sass";

interface Window {
  appAPI: import('../preload/bootstrap-preload').BootstrapAPI;
}
