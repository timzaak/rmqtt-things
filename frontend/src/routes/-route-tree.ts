import { rootRoute } from './__root'
import { indexRoute } from './index'
import { productsIndexRoute } from './products/index'
import { productsCreateRoute } from './products/create'
import { productsEditRoute } from './products/edit.$id'
import { devicesIndexRoute } from './devices/index'
import { devicesShowRoute } from './devices/show.$id'
import { validTemplatesIndexRoute } from './valid-templates/index'
import { validTemplatesCreateRoute } from './valid-templates/create'
import { validTemplatesEditRoute } from './valid-templates/edit.$id'
import { validTemplatesShowRoute } from './valid-templates/show.$id'
import { certsIndexRoute } from './certs/index'
import { certsCreateRoute } from './certs/create'
import { certsShowRoute } from './certs/show.$id'
import { otaIndexRoute } from './ota/index'
import { otaCreateRoute } from './ota/create'
import { otaEditRoute } from './ota/edit.$id'
import { otaShowRoute } from './ota/show.$id'

export const routeTree = rootRoute.addChildren([
  indexRoute,
  productsIndexRoute,
  productsCreateRoute,
  productsEditRoute,
  devicesIndexRoute,
  devicesShowRoute,
  validTemplatesIndexRoute,
  validTemplatesCreateRoute,
  validTemplatesEditRoute,
  validTemplatesShowRoute,
  certsIndexRoute,
  certsCreateRoute,
  certsShowRoute,
  otaIndexRoute,
  otaCreateRoute,
  otaEditRoute,
  otaShowRoute,
])
