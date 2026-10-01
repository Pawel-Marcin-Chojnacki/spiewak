# Co warto przejąć z innych odtwarzaczy — research Śpiewaka

Data: 1 października 2026. Cel: priorytety funkcji lokalnego, natywnego odtwarzacza Windows; późniejszy projekt UI/UX ma być oparty na konkretnych zachowaniach użytkowników.

## Wniosek produktowy

Śpiewak powinien łączyć bezpośrednie odtwarzanie folderów, przewidywalną kolejkę i wygodę znajdowania muzyki. Rozbudowane narzędzia biblioteczne mają wspierać słuchanie, bez konieczności konfigurowania całego programu. To wniosek z poniższych źródeł i wymagań projektu, nie wynik badania reprezentatywnego.

Najbardziej przydatne wzorce: kontrola foobara, zarządzanie kolekcją MusicBee, bezpośredniość 1by1, spokojna prezentacja Tauona/Dopamine, ciągłość słuchania Plexampa oraz jawne kolejki Musicolet. Nie kopiujemy całych aplikacji ani ich technologii.

## Metoda i ograniczenia

- Funkcje potwierdzam dokumentacją producentów lub repozytoriami autorów. Oceny użytkowników pochodzą z konkretnych publicznych dyskusji; odróżniam je od marketingu.
- Wybrałem dziesięć wzorców: dziewięć desktopowych, w tym Plexamp zależny od serwera, oraz Musicolet jako mobilny wzorzec kolejki. To dobór celowy dla projektu, nie ranking rynku.
- Nowsze wypowiedzi z 2025–2026 uzupełniam starszymi, gdy opisują konkretny, nadal istotny workflow. Datę podaję przy dowodzie. Stare zgłoszenie błędu nie dowodzi, że błąd istnieje w aktualnej wersji.
- Wpisy i głosy Reddit nie są reprezentatywną próbą, testem wydajności ani dowodem przewagi jakości dźwięku. Nie wyliczam procentów popularności i nie utożsamiam liczby funkcji z łatwością użycia.
- Nie przeprowadziłem porównawczych instalacji i pomiarów wszystkich tych programów. Obsługę kodeków, gapless, pamięć i stabilność Śpiewaka trzeba mierzyć osobno.
- Strona MusicBee zawiera też stare odniesienia do Groove/Windows Phone i pomiar RAM na niewielkiej bibliotece. Nie traktuję ich jako aktualnego benchmarku ani działającej integracji.

## 1. MusicBee — zarządzanie kolekcją i własne widoki

**Funkcje potwierdzone:** edycja i automatyczne uzupełnianie tagów, skórki, synchronizacja, WASAPI/ASIO, gapless, normalizacja głośności i narzędzia konwersji. [Oficjalna strona](https://getmusicbee.com/).

**Co chwalą użytkownicy:** w dyskusji „What's your favourite feature…” (16 marca 2025) wymieniają Auto-DJ przywracający zapomniane utwory, automatyczne playlisty według roku/formatu/tagów, własne panele i widok ostatnio dodanych albumów. Ważna jest możliwość oglądania swoich zbiorów na kilka sposobów, bez ręcznego przebudowywania playlist. [Dyskusja](https://www.reddit.com/r/musicbee/comments/1jcgpxw/whats_your_favourite_feature_of_musicbee_i_feel/).

**Tarcie:** dyskusja z 28 października 2023 pokazuje, że bogactwo funkcji może oznaczać zatłoczony UI i rozczarowanie obsługą tekstów; inni uczestnicy doceniają konfigurację i złożone tagowanie. [Źródło](https://www.reddit.com/r/Piracy/comments/17ikh0j). Z kolei użytkownik z 30 maja 2026 opisuje ograniczenia aktualizacji playlist z zewnętrznych plików. To argument za interoperacyjnością, nie dowód braku eksportu. [Źródło](https://www.reddit.com/r/musicbee/comments/1trpk52/automatically_update_playlists_from_a_file/).

**Dla Śpiewaka:** ostatnio dodane, ulubione, proste reguły playlist i później M3U8. Więcej opcji powinno być dostępnych stopniowo; domyślnego ekranu nie projektujemy jako panelu administracyjnego.

## 2. foobar2000 — kontrola, tagowanie, lekkość

**Potwierdzone:** gapless, ReplayGain, zaawansowane tagowanie, układ i skróty do konfiguracji, dekodery oraz rozszerzenia. Autor deklaruje brak telemetrii. [Oficjalne funkcje](https://www.foobar2000.org/?page=Overview).

**Chwalone:** użytkownicy z 19–20 sierpnia 2026 doceniają lekkość, narzędzia ID3 i zbiorczą poprawę Album Artist przy współpracach wielu artystów. W tym samym wątku ktoś uznaje obsługę za prostą, a inny za trudną i preferuje wygląd MusicBee. Ta sprzeczność jest istotna. [Dyskusja](https://www.reddit.com/r/Piracy/comments/1vszvcm/best_offline_music_player/). Wątek z 13 stycznia 2024 wskazuje nowe tagi i własne widoki jako konkretny powód pozostania przy foobarze. [Źródło](https://www.reddit.com/r/musichoarder/comments/195jy6i).

**Dla Śpiewaka:** szybkie operacje na zaznaczeniu, prawdziwe skróty i kontrola nad kolejką. Album Artist nie może zastępować folderów. Nie wymagamy instalowania skórek czy dodatków, żeby zacząć słuchać; subiektywne pochwały lekkości zamieniamy na własne pomiary 1k/10k/100k.

## 3. AIMP — wygodne playlisty i funkcje użytkowe

**Potwierdzone:** równoległe playlisty, jedna-/dwuwierszowe pozycje i ustawienia per playlista, wyszukiwanie między playlistami, playlisty oparte na folderach, ReplayGain, efekty oraz skórki z obsługą High DPI. [Oficjalne funkcje](https://aimp.ru/?do=features).

**Chwalone:** w dyskusji o wyglądzie z 15 lutego 2026 użytkownicy polecają AIMP i konkretną skórkę A4. To dowód preferencji wyglądu w tym wątku, nie dowód, że wszystkie wymienione wyżej funkcje są szczególnie cenione. [Źródło](https://www.reddit.com/r/audiophile/comments/1r5gutv/which_is_the_best_music_player_with_the_cleanest/).

**Dla Śpiewaka:** gęstość wierszy, skalowalny tekst i możliwość pracy nad listą podczas odtwarzania innej. Wygodne szukanie i zarządzanie listą jest bardziej użyteczne na początku niż duży zestaw DSP. Nie powtarzamy twierdzeń o „lepszym brzmieniu” bez pomiarów i porównania ustawień wyjścia.

## 4. MediaMonkey — duże biblioteki i bezpieczeństwo operacji

**Potwierdzone:** automatyczne i ręczne tagowanie, duplikaty, AutoPlaylists, Auto-DJ, synchronizacja, automatyczna organizacja oraz tryb jukebox ograniczający możliwość zmiany biblioteki. [Oficjalne funkcje](https://en.mediamonkey.com/windows).

**Chwalone:** wypowiedź z 20 czerwca 2026 opisuje długotrwałe używanie ocen, inteligentnych playlist i synchronizacji na Androidzie. Autor pierwotnego pytania szczególnie ceni stałe położenie kontroli oceniania i brak szukania jej w kolejnych menu. [Źródło](https://www.reddit.com/r/mediamonkey/comments/1u7p9xn/so_whats_the_mm_app_like_how_is_its_functionality/). W dyskusji z 4 listopada 2025 pojawia się uznanie dla organizacji i współpracy z konkretną kartą audio; autor później rozwiązał ten sam problem w MusicBee, więc nie przypisuję wyłącznej przewagi MediaMonkey. [Źródło](https://www.reddit.com/r/musichoarder/comments/1oo8v95/mediamonkey_is_for_me_the_best_music_player_for/).

**Tarcie kolejki:** szczegółowy komentarz z maja 2024 opisuje, że kolejne „play next” potrafią odwrócić zamierzoną kolejność i mieszać źródłową listę. [Źródło](https://www.reddit.com/r/fantanoforever/comments/1cqv78v).

**Dla Śpiewaka:** ulubione/ocena w stałym miejscu; zwykłe słuchanie nie powinno wymagać znajomości tagów. Operacje playlisty i kolejki oddzielamy od fizycznych operacji na plikach. Automatyczna reorganizacja istniejącej muzyki pozostaje wyłączona bez jawnej zgody.

## 5. Plexamp — ciągłość słuchania i odkrywanie własnej muzyki

**Potwierdzone:** gapless, wyrównanie głośności, Sweet Fades, cache i stacje; wybrane funkcje analizy podobieństwa oraz Guest DJ są powiązane z Plex Pass. Własna kolekcja wymaga Plex Media Server. [Oficjalna strona](https://www.plex.tv/plexamp/).

**Chwalone:** opis użycia Guest DJ z 13 grudnia 2022 pokazuje konkretne scenariusze: znany album poszerzany o pasujące utwory, świeżo dodana muzyka i powrót do pierwotnej kolejki po czasowej zmianie sposobu słuchania. [Źródło](https://www.reddit.com/r/plexamp/comments/zktdpp). W dyskusji z 23 listopada 2023 użytkownik opisuje ponowne odkrywanie dużej własnej kolekcji dzięki DJ Stretch. [Źródło](https://www.reddit.com/r/plexamp/comments/181zwk0).

**Tarcie:** wątek z 1 lipca 2026 jednocześnie chwali przejścia i prosi o ich wyłączenie dla playlist odtwarzanych w ustalonej kolejności. [Źródło](https://www.reddit.com/r/plexamp/comments/1ukr2ol/sweet_fades/).

**Dla Śpiewaka:** płynność i prosty powrót do słuchania są wartościowe. Albumy potrzebują prawdziwego gapless; opcjonalny crossfade powinien mieć jasny zakres. Odkrywanie zapomnianej muzyki można zacząć od historii i lokalnych reguł. Nie dokładamy serwera, konta, AI ani usług streamingowych do początkowych wydań.

## 6. Tauon — czytelna muzyka i mało wizualnego hałasu

**Potwierdzone:** drag/drop, gapless, CUE, automatyczne motywy i konfigurowalne elementy interfejsu. Projekt publikuje również instalator Windows. [Repozytorium autora](https://github.com/Taiko2k/Tauon).

**Chwalone:** wątek z 7 maja 2026 wyraźnie docenia uporządkowany, estetyczny wygląd. [Źródło](https://www.reddit.com/r/linuxaudio/comments/1t64nko/tauon_music_player_wow/). Osobny wątek z 12 kwietnia 2026 dotyczy potrzeby ładnego odtwarzacza, który nadal pozwala przeglądać i odtwarzać foldery; Tauon pojawia się jako propozycja. [Źródło](https://www.reddit.com/r/linux4noobs/comments/1sjgnvu/modern_music_player_recommendations_please/).

**Dla Śpiewaka:** okładka, hierarchia tekstu, przestrzeń i czytelna lista zamiast wielu kart. Funkcje muszą pozostać dostępne z klawiatury i dla czytnika ekranu. Własny wygląd nie wymaga odtwarzania od zera podstawowych zachowań natywnych kontrolek.

## 7. Dopamine — prosty początek

**Potwierdzone:** autor stawia na prostotę i estetykę; aktualna linia w repozytorium używa Electron/Angular/TypeScript. To wzorzec doświadczenia, nie akceptowalny stos dla Śpiewaka. [Repozytorium](https://github.com/digimezzo/dopamine).

**Chwalone i krytykowane:** 6 lipca 2026 użytkownik chwali prostotę i ciemny motyw, potrzebując tylko grupowania poprawnie otagowanej muzyki i słuchania. Jednocześnie przeszkadza mu konkretny element grupowania; inni wskazują ograniczenia metadanych. [Źródło](https://www.reddit.com/r/linuxquestions/comments/1up2zeq/dopamine_3_music_app_help/).

**Dla Śpiewaka:** przyjazny pierwszy start i jasna ścieżka „wybierz folder → słuchaj”. Funkcjonalności zaawansowane pozostają dostępne, ale nie dominują ekranu. Nie dodajemy ozdobnych grup, które utrudniają znalezienie albumu.

## 8. Winamp / WACUP — bezpośredniość i pamięć mięśniowa

Nie utożsamiam aktualnej marki Winamp z klasycznym odtwarzaczem. WACUP rozwija doświadczenie kompatybilne z klasycznym Winampem; możliwości i zgodność różnią się między wariantami. [Oficjalny projekt](https://getwacup.com/).

**Chwalone:** wpis z 28 lutego 2025 docenia zachowane skórki oraz usprawnienia codziennej obsługi; pod nim są również zgłoszenia problemów i wskazówki naprawy konkretnej wersji. [Źródło](https://www.reddit.com/r/WACUP/comments/1j0cm64). Wpis z 13 października 2025 pokazuje, że dodawanie do kolejki z menu Eksploratora bywa podstawowym sposobem korzystania, a problem z tą integracją blokuje zmianę programu. [Źródło](https://www.reddit.com/r/WACUP/comments/1o5v5v1). Wymagania użytkownika z 7 lipca 2026: szybkie ładowanie plików, stabilność, obsługa formatów i większe fonty — bez potrzeby biblioteki czy wizualizacji. [Źródło](https://www.reddit.com/r/WACUP/comments/1upiwvd/which_is_the_latest_most_stable_version_of_wacup/).

**Dla Śpiewaka:** przeciągnięcie plików, skróty, później skojarzenia/menu Eksploratora i skalowalne elementy. Rozmiar fontów i trafienie w przycisk są funkcjami użytkowymi; nie sprowadzamy ich do dekoracji. Retroskórki i pluginy nie są celem pierwszego wydania.

## 9. Musicolet / 1by1 — wartość prostego modelu lokalnego

**Musicolet:** oficjalnie lokalny Android bez uprawnienia Internet, z wieloma kolejkami, folderami, multizaznaczeniem, playlistami, tekstami i timerami. [Autor](https://krosbits.in/musicolet/). W dyskusji z 30 grudnia 2022 użytkownicy szczególnie cenią powrót do różnych zachowanych kolejek i widoczność rzeczywistej kolejki także przy shuffle. [Źródło](https://www.reddit.com/r/androidapps/comments/zyz07t/i_tried_out_20_music_players_for_android_heres/). Kontrprzykład z 20 lutego 2023: automatyczne tworzenie kolejki całego albumu, gdy ktoś chce jednego utworu, jest irytujące. [Źródło](https://www.reddit.com/r/androidapps/comments/116wp4h).

**1by1:** autor opisuje odtwarzacz bezpośrednio oparty na katalogach, odtwarzanie folderów bez obowiązkowej budowy playlist i zapamiętywanie miejsca słuchania. [Oficjalna strona](https://mpesch3.de/1by1.html). Użytkownik z czerwca 2025 chwali go właśnie jako narzędzie jednego celu dla muzyki uporządkowanej artysta/album. [Źródło](https://www.reddit.com/r/musichoarder/comments/1lbvd9y).

**Dla Śpiewaka:** zachowanie kolejki i pozycji po restarcie, oddzielne „odtwórz utwór”, „odtwórz folder”, „dodaj”, „odtwórz następne”. Przeglądanie nie powinno samo zmieniać odtwarzania. Wiele niezależnych kolejek to późniejsza opcja, zależna od rzeczywistego workflow domowników.

## Mapa priorytetów — nasza decyzja, nie ranking popularności

| Obszar | Wartość i dowód | Stan Śpiewaka | Priorytet / kryterium ukończenia |
|---|---|---|---|
| Wierne foldery | 1by1, wątki o przeglądaniu folderów | jest | P0: żaden tag nie zmienia hierarchii; root offline zachowuje cache |
| Kolejka przewidywalna | pochwały Musicolet, krytyka MM | mieszana kolejka i edycja są | P0: jawny zakres Play; duplikaty i shuffle działają; kolejne prośby „następne” mają ustaloną semantykę |
| Wyszukiwanie i zaznaczenie | foobar/MusicBee; konkretne codzienne operacje | FTS i batch Play/Queue są | P0: polskie znaki, szybkie wyniki, brak blokowania UI, brak przeskoku kolejki podczas szukania |
| Trwałe playlisty | MusicBee/MM/Musicolet | są, z soft-delete/restore | P0: brakujący plik pozostaje widoczny; rename/move nie zrywa odwołań |
| Synchronizacja plików | biblioteka ma odpowiadać temu, co w Explorerze | ręczny rescan/startup reconcile | P0: obserwacja zmian; debounce; stabilne ID przy rename/move na wspieranym systemie |
| Przywrócenie słuchania | 1by1/Musicolet | brak | P1: kolejka, kolejność, volume i pozycja wracają; start bez niespodziewanego autoplay |
| Ulubione / ostatnio dodane / historia | MusicBee/MM/Plexamp | brak | P1: jeden łatwo dostępny gest, bez edycji tagów i bez reorganizacji plików |
| Wygoda głośności i przejść | ReplayGain, Plexamp, wymagania lokalnych użytkowników | volume/mute są | P1: ReplayGain track/album; gapless potwierdzony testem, nie deklaracją; crossfade opt-in |
| Globalne media keys / urządzenie | foobar, WACUP, konkretne historie MM | brak | P1: działają poza oknem; zmiana/utrata urządzenia daje czytelny stan |
| Okładki i prosta informacja | Tauon/Dopamine; przeglądanie MM | lokalna okładka folderu | P1: cache embedded artwork; kontrola rozmiaru; neutralny placeholder |
| Drag/drop i M3U8 | Tauon/WACUP; przenoszenie bibliotek MusicBee | brak | P1: pliki i foldery z Explorera; eksport/import UTF-8, względne ścieżki, jawne brakujące pliki |
| Ręczne tagi / bezpieczny Inbox | potrzeby projektu i narzędzia biblioteczne | brak | P1/P2: preview → apply → undo; dostawcy i klucze konfigurowane jawnie |
| Smart playlists | powtarzalne zastosowania MusicBee/MM | brak | P2: mały zestaw zrozumiałych reguł i podgląd wyniku |
| Teksty / sleep timer / A-B | obecne u konkurencji, zależne od osoby i sposobu słuchania | brak | P2 kandydaci; awans dopiero po potwierdzeniu domowych wymagań |
| Sonic AI, streaming, serwer, pluginy | Plexamp i duże kombajny | poza zakresem | Zachować non-goals planu; nie rozszerzać produktu z samego podobieństwa do konkurencji |

## Wymagania drugiego domownika

Potwierdzone: właściciel prosi uwzględnić wymagania żony. W dostarczonym planie i dostępnej historii projektu nie ma ich osobnej listy; poprosiłem o konkretne uwagi. **Nie zakładamy, że jest początkującą użytkowniczką, używa telefonu, potrzebuje karaoke lub chce automatycznego tagowania.**

Do uzupełnienia po jej odpowiedzi:

- skąd bierze muzykę i czy słucha albumów, pojedynczych utworów czy playlist;
- jak rozpoznaje i znajduje nagranie: nazwa, wykonawca, okładka, folder, fragment tekstu;
- co musi być na wierzchu podczas słuchania;
- czy komputer i kolejka są wspólne; czy trzeba zachowywać oddzielne ulubione/konteksty;
- jak chce dodawać nowe pliki i które automatyczne zmiany akceptuje;
- oczekiwania dotyczące wielkości elementów, klawiatury, tekstów, timerów lub nauki fragmentu.

To nie lista jej wymagań. To pytania do walidacji. Do tego czasu rozwijamy wspólne, odwracalne podstawy odtwarzania i biblioteki.

## Scenariusze akceptacji przed finalnym UI/UX

1. Dodaj folder, znajdź utwór po polskiej nazwie i odtwórz go bez konfigurowania tagów.
2. Podczas słuchania przeglądaj i szukaj; nic nie przerywa bieżącego utworu.
3. Dodaj A, następnie B jako „następne”; wynik i rzeczywista kolejność są jawne i zgodne z opisem polecenia. Przetestuj również shuffle i duplikaty.
4. Zapisz kolejkę, zamknij program i wróć do playlisty; nic nie odtwarza się samoczynnie bez ustawienia autoplay.
5. Zmień nazwę pliku/albumu w Explorerze: aktualizuje się biblioteka i istniejąca playlista; root offline nie usuwa jej zawartości.
6. Odtwarzaj album z łączącymi się ścieżkami; pomiar potwierdza brak nieplanowanej przerwy.
7. Uszkodzony plik, utrata urządzenia i skan w tle nie zamrażają UI; błędy odnoszą się do konkretnej czynności.
8. Przy normalnym skalowaniu i dużym tekście wszystkie stałe elementy transportu są czytelne i dostępne z klawiatury.
9. Przejdź konkretne, potwierdzone przez drugiego domownika zadania; zapisz problemy przed dopracowaniem wizualnym.

## Kierunek dla późniejszego agenta UI/UX

- Punktem wejścia jest `windows/Player/Design/Theme.xaml`: kolory semantyczne i wspólne style. Bez logiki domenowej w motywie.
- Projektować prawdziwe stany: folder z albumem, luźne utwory, wyniki wyszukiwania, duplikaty w kolejce, plik offline, skanowanie i pusta playlista.
- Stały transport, jawny Now Playing i lista faktycznie nadchodzących utworów. Widok zaawansowany i inspector nie powinny wypierać głównego zadania.
- Zachować focus, screen reader, high contrast i wirtualizację. Testować w rzeczywistym Windows; screenshot samej pustej ramy nie dowodzi dobrego UX.
- Własny, spokojny wygląd odtwarzacza; unikać panelu pełnego kafelków. Dopasowanie do gustu obojga użytkowników pozostaje osobnym etapem.
