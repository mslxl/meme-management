import { lazy } from 'react'
import { createBrowserRouter } from 'react-router-dom'

const WelcomeLayout = lazy(() => import('./pages/welcome/layout'))
const WelcomePage = lazy(() => import('./pages/welcome/page'))

const DashboardPage = lazy(() => import('./pages/dashboard/page'))

const AddLayout = lazy(() => import('./pages/add/layout'))
const AddImagePage = lazy(() => import('./pages/add/image/page'))
import AddImagePageLoader from './pages/add/image/loader'
const AddTextPage = lazy(() => import('./pages/add/text/page'))

const EditLayout = lazy(() => import('./pages/edit/layout'))
const EditImagePage = lazy(() => import('./pages/edit/image/page'))
import EditPageLoader from './pages/edit/loader'
const EditTextPage = lazy(() => import('./pages/edit/image/page'))

const MemePreviewPage = lazy(() => import('./pages/meme/page'))
import MemePreivewPageLoader from './pages/meme/loader'

const SettingsLayout = lazy(() => import('./pages/settings/layout'))
const SettingsPage = lazy(() => import('./pages/settings/page'))

const routes = createBrowserRouter([
  {
    path: '/',
    element: <WelcomeLayout />,
    children: [{ path: '/', element: <WelcomePage /> }]
  },
  {
    path: '/dashboard',
    children: [
      {
        path: '/dashboard/',
        element: <DashboardPage />
      },
      {
        path: '/dashboard/s/:search',
        element: <DashboardPage />
      },
      {
        path: '/dashboard/fav/',
        element: <DashboardPage />
      },
      {
        path: '/dashboard/fav/s/:search',
        element: <DashboardPage />
      },
      {
        path: '/dashboard/trash',
        element: <DashboardPage />
      }
    ]
  },
  {
    path: '/preview/:id',
    element: <MemePreviewPage />,
    loader: MemePreivewPageLoader,
  },
  {
    path: '/add',
    element: <AddLayout />,
    children: [
      {
        path: '/add/image',
        element: <AddImagePage />,
        loader: AddImagePageLoader
      },
      {
        path: '/add/text',
        element: <AddTextPage />
      }
    ]
  },
  {
    path: '/edit',
    element: <EditLayout />,
    children: [
      {
        path: '/edit/image/:id',
        element: <EditImagePage />,
        loader: EditPageLoader
      },
      {
        path: '/edit/text/:id',
        element: <EditTextPage />,
        loader: EditPageLoader
      }
    ]
  },
  {
    path: '/settings',
    element: <SettingsLayout />,
    children: [
      {
        path: '/settings',
        element: <SettingsPage />
      }
    ]
  }
])

export default routes