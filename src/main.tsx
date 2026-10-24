import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Barre } from './barre/Barre';
import './styles.css';

const racine = document.getElementById('racine');
if (racine === null) {
  throw new Error('Élément #racine introuvable dans index.html.');
}
createRoot(racine).render(
  <StrictMode>
    <Barre />
  </StrictMode>,
);
