import type { UpdateStatus } from '@/domain/updater/apis/updater.dto';
import call from '@/shared/api';

export async function checkUpdate() {
  return call<UpdateStatus>('check_update');
}

export async function getUpdateStatus() {
  return call<UpdateStatus>('get_update_status');
}

export async function installUpdate() {
  return call<void>('install_update_and_restart');
}
