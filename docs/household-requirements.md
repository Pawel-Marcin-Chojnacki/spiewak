# Wymagania domowe Śpiewaka

Potwierdzone przez właściciela 1 października 2026, po researchu odtwarzaczy. Ten dokument uzupełnia master-plan.md i ma pierwszeństwo przed wcześniejszymi domysłami w researchu. Wymagania przekazał właściciel w imieniu żony; nie przeprowadziliśmy jeszcze jej testów użytkowych.

## Potwierdzone oczekiwania

1. Bardzo prosty odtwarzacz: otworzyć swój folder, znaleźć muzykę i słuchać. Zaawansowane opcje dostępne wtedy, gdy są potrzebne.
2. Kolekcja na dysku jest podstawą organizacji. Foldery, ich nazwy i położenie muszą być szanowane. Metadane wzbogacają prezentację i wyszukiwanie.
3. Pojedyncze utwory, nawet z tagiem albumu, nie mają tworzyć wielkich albumów na stronie głównej. Zbiór niezależnych utworów pozostaje zbiorem w swoim folderze; widok albumów ma wynikać z rzeczywistych katalogów. Nie tworzymy nowych albumów/wykonawców na podstawie każdego tagu Artist/Album.
4. Automatyczne pobieranie okładek.
5. Automatyczne wyszukiwanie i uzupełnianie tagów.
6. Porządkowanie MP3 pobranych z YouTube, szczególnie nazw i metadanych. Czy „czyszczenie” obejmuje również obróbkę audio, pozostaje do doprecyzowania; obecny zakres nie zmienia sygnału audio.

Dla całego produktu potwierdzony jest też wcześniejszy kierunek właściciela: własny, spokojny wygląd, bez kafelków przypominających Windows 10; finalny UI/UX dopracujemy osobno. Nie zakładamy braku doświadczenia technicznego, potrzeby karaoke, profili domowników, timerów ani mobilnej aplikacji.

## Przełożenie na zachowanie aplikacji

- Foldery zawsze dostępne jako główna nawigacja. Dodanie tagów lub okładki do utworu nie przenosi go do innego folderu.
- Automatyczna ocena prezentacji jest konserwatywna: jeden plik to luźny utwór. Kilka utworów ze wspólnym albumem i wykonawcą albumowym, kompletem różnych dodatnich numerów ścieżek i bez podfolderów może dostać prezentację albumową. To heurystyka, nie dowód kompletności wydania.
- Folder z utworami i podfolderami ma prezentację mieszaną. Niejasne dane pozostają jako luźne utwory lub zwykła kolekcja.
- Użytkownik może wybrać Album, Artist folder, Loose tracks, Mixed folder, Compilation lub My collection i wrócić do Automatic. Wybór zapisujemy przy stabilnym ID folderu w bazie aplikacji; skanowanie go nie nadpisuje.
- Luźne, mieszane i zwykłe kolekcje mają zwarty nagłówek i listę plików. Na przyszłym Home mogą mieć spokojną sekcję/listę z boku. Duże albumowe okładki są przeznaczone dla folderów Album/Compilation.
- Okładki albumów mogą być wspólne dla folderu. Luźne utwory potrzebują własnych okładek; nie rozciągamy okładki jednej piosenki na przypadkowy zbiór.
- Więcej metadanych nie powinno komplikować głównego ekranu słuchania. W szczególności stan Inbox/Needs Review nie powinien blokować odtwarzania.

## Automatyzacja metadanych — priorytet P1

Automatyczne okładki/tagi i porządkowanie nowych MP3 są potwierdzoną potrzebą produktu. Nie są już opcjonalnym dodatkiem wynikającym wyłącznie z porównania konkurencji.

Kolejność wdrożenia:

1. Odczyt wbudowanych okładek i cache; poprawny wybór okładki folderu albumowego versus pojedynczego utworu.
2. Rozpoznanie nazw/tagów, zachowanie oryginalnych danych i propozycje poprawek. Usuwanie wyłącznie znanego szumu, np. „Official Video” czy „320kbps”; zachowanie oznaczeń wersji Live/Remix/Acoustic i znaczącego tekstu w nawiasach.
3. Wyszukiwanie kandydatów, rozpoznanie nagrania i pobranie pasującej okładki. Kandydaci do integracji: MusicBrainz, AcoustID i Cover Art Archive; patrz smart-inbox.md. Sama zbieżna nazwa lub wynik wyszukiwarki nie dowodzą tożsamości nagrania.
4. Operacje rename/tag/artwork ze stanem oryginalnym, historią i sprawdzonym Undo, bez nadpisania istniejącego innego pliku. Dopiero na tej podstawie automatyczne zastosowanie pewnych dopasowań w wybranych folderach Inbox.
5. Niejednoznaczne nagranie pozostaje w swoim folderze jako Needs Review. Brak okładki lub brak odpowiedzi dostawcy nie zatrzymuje słuchania.

Zwykły skaner pozostaje tylko do odczytu. Automatyzacja dotyczy wybranych nowych plików/Inbox. Nie reorganizuje istniejących albumów, nie scala katalogów, nie usuwa muzyki i nie tworzy struktury katalogów na podstawie pobranych tagów.

## Scenariusze akceptacji

- `Pink Floyd/The Wall/` pozostaje tym samym katalogiem przed i po wzbogaceniu metadanych. Ręcznie wybrane Loose tracks/My collection jest respektowane nawet przy wspólnym albumie w tagach.
- `Random Songs/Haddaway - What Is Love.mp3` pozostaje pojedynczym utworem w Random Songs, również gdy ma tag albumu i własną okładkę. Nie powstaje duży album Haddaway na Home.
- Dziesięć MP3 z różnymi tagami albumów w jednym katalogu nie staje się dziesięcioma albumami w głównej nawigacji.
- Zmiana nazwy albumowego katalogu w Eksploratorze zachowuje ręczny typ folderu i powiązania playlisty.
- Nowy MP3 ze znanym szumem w nazwie dostaje poprawne tagi/okładkę przy pewnym dopasowaniu; na każdym etapie można odtworzyć oryginalne dane i lokalizację. Nagranie Live/Remix nie jest błędnie sprowadzone do wersji studyjnej.
- Przy niepewnym dopasowaniu plik pozostaje bez zmian, widoczny i odtwarzalny; aplikacja pokazuje prostą możliwość sprawdzenia propozycji.

## Aktualna realizacja

Foldery, odtwarzanie, wyszukiwanie, playlisty i synchronizacja są zaimplementowane. Klasyfikacja/prezentacja folderów i zapisany wybór użytkownika są zaimplementowane i sprawdzone testami Rust/C#; nowe zachowanie wizualne wymaga jeszcze oceny na odblokowanym Macu/Windows. Odczyt okładek wbudowanych i folderowych, ograniczony cache miniatur oraz rozdzielenie obrazów albumu i pojedynczych utworów są zaimplementowane i sprawdzone natywnie na Windows (szczegóły: ARTWORK.md). Wygląd obrazów w oknie i pełna obsługa użytkowa pozostają do sprawdzenia. Home jako osobny widok, integracje dostawców, automatyczne pobieranie okładek/tagów, zmiany plików i Undo nadal wymagają implementacji. Nie uznajemy ich za gotowe na podstawie samego dokumentu.
